use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use tauri::{Manager, State};

// ------------------------- Data model -------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Workspace {
    id: String,
    name: String,
    path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Ide {
    id: String,
    name: String,
    /// Full path to the application bundle, e.g. /Applications/PhpStorm.app
    path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    workspaces: Vec<Workspace>,
    #[serde(default)]
    ides: Vec<Ide>,
    #[serde(default)]
    default_ide_id: Option<String>,
    /// Global hotkey accelerator (e.g. "CmdOrCtrl+Shift+M"). None = default.
    #[serde(default)]
    shortcut: Option<String>,
    /// Last-opened info per project path.
    #[serde(default)]
    recents: std::collections::HashMap<String, Recent>,
}

fn default_shortcut() -> String {
    "CmdOrCtrl+Shift+M".to_string()
}

/// Per-project usage info, keyed by absolute project path.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Recent {
    /// Unix seconds of the last time the project was opened from naeasy.
    last_opened: u64,
    /// Name of the IDE it was last opened with.
    ide: String,
}

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A node in the project tree. Leaves with `is_repo = true` are openable Git projects.
#[derive(Debug, Clone, Serialize)]
struct TreeNode {
    name: String,
    path: String,
    is_repo: bool,
    children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Serialize)]
struct WorkspaceView {
    id: String,
    name: String,
    path: String,
    tree: TreeNode,
    repo_count: usize,
}

#[derive(Debug, Clone, Serialize)]
struct AppData {
    workspaces: Vec<WorkspaceView>,
    ides: Vec<Ide>,
    default_ide_id: Option<String>,
    shortcut: String,
    recents: std::collections::HashMap<String, Recent>,
}

struct AppState {
    config: Mutex<Config>,
    config_path: Mutex<PathBuf>,
}

// ------------------------- Persistence -------------------------

fn load_config(path: &Path) -> Config {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

fn save_config(path: &Path, config: &Config) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

fn gen_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{:x}-{:x}", nanos, n)
}

// ------------------------- Scanning -------------------------

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "vendor",
    "storage",
    "dist",
    "build",
    "target",
    ".next",
    ".cache",
];

const MAX_DEPTH: usize = 6;

/// Recursively collect directories that contain a `.git` entry.
/// Does not descend into a repo once found (repos are leaves).
fn find_repos(root: &Path) -> Vec<PathBuf> {
    let mut repos = Vec::new();
    walk(root, 0, &mut repos);
    repos.sort();
    repos
}

fn walk(dir: &Path, depth: usize, repos: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH {
        return;
    }
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut is_repo = false;

    for entry in entries.flatten() {
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            is_repo = true;
            // keep scanning entries to finish, but we won't descend
        }
        subdirs.push(entry.path());
    }

    if is_repo {
        repos.push(dir.to_path_buf());
        return; // a repo is a leaf — do not descend
    }

    for sub in subdirs {
        let name = sub
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if name.starts_with('.') {
            continue;
        }
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        walk(&sub, depth + 1, repos);
    }
}

/// Build a nested tree from repo paths relative to `root`.
fn build_tree(root: &Path, repos: &[PathBuf]) -> TreeNode {
    let mut tree = TreeNode {
        name: root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string()),
        path: root.to_string_lossy().to_string(),
        is_repo: false,
        children: Vec::new(),
    };

    for repo in repos {
        let rel = match repo.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let parts: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
            .collect();

        let mut node = &mut tree;
        let mut acc = root.to_path_buf();
        let last = parts.len().saturating_sub(1);
        for (i, part) in parts.iter().enumerate() {
            acc = acc.join(part);
            let idx = match node.children.iter().position(|c| &c.name == part) {
                Some(idx) => idx,
                None => {
                    node.children.push(TreeNode {
                        name: part.clone(),
                        path: acc.to_string_lossy().to_string(),
                        is_repo: false,
                        children: Vec::new(),
                    });
                    node.children.len() - 1
                }
            };
            node = &mut node.children[idx];
            if i == last {
                node.is_repo = true;
            }
        }
    }

    sort_tree(&mut tree);
    tree
}

fn sort_tree(node: &mut TreeNode) {
    // Folders (groups) first, then repos, each alphabetical.
    node.children.sort_by(|a, b| {
        let a_group = !a.children.is_empty();
        let b_group = !b.children.is_empty();
        b_group
            .cmp(&a_group)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for child in &mut node.children {
        sort_tree(child);
    }
}

fn count_repos(node: &TreeNode) -> usize {
    let mut n = if node.is_repo { 1 } else { 0 };
    for c in &node.children {
        n += count_repos(c);
    }
    n
}

fn build_view(ws: &Workspace) -> WorkspaceView {
    let root = PathBuf::from(&ws.path);
    let repos = find_repos(&root);
    let tree = build_tree(&root, &repos);
    let repo_count = count_repos(&tree);
    WorkspaceView {
        id: ws.id.clone(),
        name: ws.name.clone(),
        path: ws.path.clone(),
        tree,
        repo_count,
    }
}

fn build_app_data(config: &Config) -> AppData {
    AppData {
        workspaces: config.workspaces.iter().map(build_view).collect(),
        ides: config.ides.clone(),
        default_ide_id: config.default_ide_id.clone(),
        shortcut: config.shortcut.clone().unwrap_or_else(default_shortcut),
        recents: config.recents.clone(),
    }
}

// ------------------------- IDE detection -------------------------

/// Known IDE / editor bundle display names we try to auto-detect.
const KNOWN_IDES: &[&str] = &[
    "PhpStorm",
    "GoLand",
    "DataGrip",
    "PyCharm",
    "PyCharm Professional Edition",
    "PyCharm Community Edition",
    "IntelliJ IDEA",
    "IntelliJ IDEA Ultimate",
    "IntelliJ IDEA Community Edition",
    "WebStorm",
    "CLion",
    "RubyMine",
    "Rider",
    "RustRover",
    "Fleet",
    "Visual Studio Code",
    "VSCodium",
    "Cursor",
    "Zed",
    "Sublime Text",
    "Nova",
    "Windsurf",
];

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn scan_apps_dir(dir: &Path, found: &mut Vec<Ide>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("app") {
            continue;
        }
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if KNOWN_IDES.iter().any(|k| stem.eq_ignore_ascii_case(k)) {
            let path_str = path.to_string_lossy().to_string();
            if !found.iter().any(|i| i.path == path_str) {
                found.push(Ide {
                    id: gen_id(),
                    name: stem,
                    path: path_str,
                });
            }
        }
    }
}

fn detect_installed_ides() -> Vec<Ide> {
    let mut found: Vec<Ide> = Vec::new();
    let mut dirs: Vec<PathBuf> = vec![PathBuf::from("/Applications")];
    if let Some(home) = home_dir() {
        dirs.push(home.join("Applications"));
        // JetBrains Toolbox default launcher location
        dirs.push(home.join("Applications/JetBrains Toolbox"));
    }
    for dir in dirs {
        scan_apps_dir(&dir, &mut found);
    }
    found.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    found
}

fn ide_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

// ------------------------- Commands -------------------------

#[tauri::command]
fn get_data(state: State<AppState>) -> AppData {
    let config = state.config.lock().unwrap();
    build_app_data(&config)
}

#[tauri::command]
fn rescan(state: State<AppState>) -> AppData {
    let config = state.config.lock().unwrap();
    build_app_data(&config)
}

#[tauri::command]
fn add_workspace(state: State<AppState>, path: String) -> Result<AppData, String> {
    let p = PathBuf::from(&path);
    if !p.is_dir() {
        return Err(format!("Not a directory: {path}"));
    }
    let mut config = state.config.lock().unwrap();
    if config.workspaces.iter().any(|w| w.path == path) {
        return Err("Workspace already added".into());
    }
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.clone());
    config.workspaces.push(Workspace {
        id: gen_id(),
        name,
        path,
    });
    let cp = state.config_path.lock().unwrap();
    save_config(&cp, &config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn remove_workspace(state: State<AppState>, id: String) -> Result<AppData, String> {
    let mut config = state.config.lock().unwrap();
    config.workspaces.retain(|w| w.id != id);
    let cp = state.config_path.lock().unwrap();
    save_config(&cp, &config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn detect_ides(state: State<AppState>) -> Result<AppData, String> {
    let detected = detect_installed_ides();
    let mut config = state.config.lock().unwrap();
    for ide in detected {
        if !config.ides.iter().any(|i| i.path == ide.path) {
            config.ides.push(ide);
        }
    }
    if config.default_ide_id.is_none() {
        config.default_ide_id = config.ides.first().map(|i| i.id.clone());
    }
    let cp = state.config_path.lock().unwrap();
    save_config(&cp, &config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn add_ide(state: State<AppState>, path: String) -> Result<AppData, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("Path does not exist: {path}"));
    }
    let mut config = state.config.lock().unwrap();
    if config.ides.iter().any(|i| i.path == path) {
        return Err("IDE already added".into());
    }
    let name = ide_name_from_path(&path);
    let ide = Ide {
        id: gen_id(),
        name,
        path,
    };
    let new_id = ide.id.clone();
    config.ides.push(ide);
    if config.default_ide_id.is_none() {
        config.default_ide_id = Some(new_id);
    }
    let cp = state.config_path.lock().unwrap();
    save_config(&cp, &config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn remove_ide(state: State<AppState>, id: String) -> Result<AppData, String> {
    let mut config = state.config.lock().unwrap();
    config.ides.retain(|i| i.id != id);
    if config.default_ide_id.as_deref() == Some(id.as_str()) {
        config.default_ide_id = config.ides.first().map(|i| i.id.clone());
    }
    let cp = state.config_path.lock().unwrap();
    save_config(&cp, &config)?;
    Ok(build_app_data(&config))
}

#[tauri::command]
fn set_default_ide(state: State<AppState>, id: String) -> Result<AppData, String> {
    let mut config = state.config.lock().unwrap();
    if !config.ides.iter().any(|i| i.id == id) {
        return Err("Unknown IDE".into());
    }
    config.default_ide_id = Some(id);
    let cp = state.config_path.lock().unwrap();
    save_config(&cp, &config)?;
    Ok(build_app_data(&config))
}

/// Open a project folder in the chosen (or default) IDE.
#[tauri::command]
fn open_project(
    state: State<AppState>,
    project_path: String,
    ide_id: Option<String>,
) -> Result<(), String> {
    let mut config = state.config.lock().unwrap();
    let chosen = ide_id.or_else(|| config.default_ide_id.clone());
    let ide = match chosen.and_then(|id| config.ides.iter().find(|i| i.id == id).cloned()) {
        Some(ide) => ide,
        None => return Err("No IDE selected. Add an IDE first.".into()),
    };

    if !Path::new(&project_path).exists() {
        return Err(format!("Project not found: {project_path}"));
    }

    open_in_app(&ide.path, &project_path)?;

    // Remember when and with which IDE this project was opened.
    config.recents.insert(
        project_path.clone(),
        Recent {
            last_opened: now_secs(),
            ide: ide.name.clone(),
        },
    );
    let cp = state.config_path.lock().unwrap();
    let _ = save_config(&cp, &config);
    Ok(())
}

/// Reveal the project folder in Finder.
#[tauri::command]
fn reveal_in_finder(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg("-R")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
        return Ok(());
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = path;
        Err("Reveal in Finder is only supported on macOS".into())
    }
}

#[cfg(target_os = "macos")]
fn open_in_app(app_path: &str, project_path: &str) -> Result<(), String> {
    // 1. If the project is already open in this IDE, switch to its window
    //    (un-minimize + raise + focus) instead of opening it again.
    if focus_existing_window(app_path, project_path) {
        return Ok(());
    }
    // 2. Otherwise launch / open it. `open -a` also focuses an existing
    //    project window for IDEs that support it (JetBrains, VS Code, …).
    Command::new("open")
        .arg("-a")
        .arg(app_path)
        .arg(project_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch IDE: {e}"))
}

/// Map an app bundle name to the process name used by System Events.
/// Most apps match their bundle name; a few editors differ.
#[cfg(target_os = "macos")]
fn process_name_for(app_path: &str) -> String {
    let bundle = ide_name_from_path(app_path);
    match bundle.as_str() {
        "Visual Studio Code" => "Code".to_string(),
        "Visual Studio Code - Insiders" => "Code - Insiders".to_string(),
        "VSCodium" => "VSCodium".to_string(),
        other => other.to_string(),
    }
}

/// Best-effort: find a window of the running IDE whose title contains the
/// project folder name, un-minimize it, raise it and focus the app.
/// Returns true only if a matching window was focused.
///
/// Uses AppleScript / System Events. If Accessibility permission is not
/// granted the script fails and we return false (caller falls back to `open`).
#[cfg(target_os = "macos")]
fn focus_existing_window(app_path: &str, project_path: &str) -> bool {
    let proc_name = process_name_for(app_path);
    let project_name = Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if project_name.is_empty() {
        return false;
    }

    let app_q = applescript_quote(&proc_name);
    let proj_q = applescript_quote(&project_name);

    // Match windows whose title contains the project name. JetBrains and most
    // editors put the project folder name in the window title.
    let script = format!(
        r#"tell application "System Events"
    if exists (process {app}) then
        tell process {app}
            set matches to (every window whose name contains {proj})
            if (count of matches) > 0 then
                set w to item 1 of matches
                try
                    set value of attribute "AXMinimized" of w to false
                end try
                try
                    perform action "AXRaise" of w
                end try
                set frontmost to true
                return "FOCUSED"
            end if
        end tell
    end if
end tell
return "NOFOCUS""#,
        app = app_q,
        proj = proj_q
    );

    match Command::new("osascript").arg("-e").arg(&script).output() {
        Ok(out) => {
            out.status.success() && String::from_utf8_lossy(&out.stdout).contains("FOCUSED")
        }
        Err(_) => false,
    }
}

/// Quote a string as an AppleScript string literal.
#[cfg(target_os = "macos")]
fn applescript_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(target_os = "windows")]
fn open_in_app(app_path: &str, project_path: &str) -> Result<(), String> {
    Command::new(app_path)
        .arg(project_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch IDE: {e}"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_in_app(app_path: &str, project_path: &str) -> Result<(), String> {
    Command::new(app_path)
        .arg(project_path)
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("Failed to launch IDE: {e}"))
}

// ------------------------- Open-project detection -------------------------

/// Which of the given project folder names are currently open in an IDE.
/// Returns a map of name -> IDE process name. Best-effort (needs Accessibility).
#[tauri::command]
fn scan_open_projects(names: Vec<String>) -> std::collections::HashMap<String, String> {
    #[cfg(target_os = "macos")]
    {
        scan_open_macos(&names)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = names;
        std::collections::HashMap::new()
    }
}

#[cfg(target_os = "macos")]
fn scan_open_macos(names: &[String]) -> std::collections::HashMap<String, String> {
    use std::collections::HashMap;
    let mut result = HashMap::new();

    let procs = [
        "PhpStorm",
        "GoLand",
        "DataGrip",
        "PyCharm",
        "IntelliJ IDEA",
        "WebStorm",
        "CLion",
        "RubyMine",
        "Rider",
        "RustRover",
        "Fleet",
        "Code",
        "Cursor",
        "Zed",
        "Windsurf",
    ];
    let proc_list = procs
        .iter()
        .map(|p| format!("\"{p}\""))
        .collect::<Vec<_>>()
        .join(", ");

    // Built via concatenation to avoid brace-escaping in format strings.
    let script = String::from("set out to \"\"\n")
        + "tell application \"System Events\"\n"
        + "  repeat with pn in {"
        + &proc_list
        + "}\n"
        + "    set pname to (pn as text)\n"
        + "    if exists (process pname) then\n"
        + "      tell process pname\n"
        + "        repeat with w in windows\n"
        + "          try\n"
        + "            set out to out & pname & tab & (name of w) & linefeed\n"
        + "          end try\n"
        + "        end repeat\n"
        + "      end tell\n"
        + "    end if\n"
        + "  end repeat\n"
        + "end tell\n"
        + "return out";

    let output = match Command::new("osascript").arg("-e").arg(&script).output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return result,
    };

    let lines: Vec<(String, String)> = output
        .lines()
        .filter_map(|l| {
            let mut parts = l.splitn(2, '\t');
            let p = parts.next()?.trim().to_string();
            let t = parts.next()?.to_string();
            Some((p, t))
        })
        .collect();

    for name in names {
        if name.is_empty() {
            continue;
        }
        for (proc, title) in &lines {
            if title.contains(name.as_str()) {
                result.insert(name.clone(), proc.clone());
                break;
            }
        }
    }
    result
}

// ------------------------- Global shortcut -------------------------

fn toggle_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let minimized = w.is_minimized().unwrap_or(false);
        let visible = w.is_visible().unwrap_or(false);
        if visible && !minimized {
            let _ = w.hide();
        } else {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

fn register_shortcut(app: &tauri::AppHandle, accel: &str) -> Result<(), String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    let shortcut: tauri_plugin_global_shortcut::Shortcut = accel
        .parse()
        .map_err(|_| format!("Invalid shortcut: {accel}"))?;
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    gs.register(shortcut).map_err(|e| e.to_string())
}

#[tauri::command]
fn set_shortcut(
    app: tauri::AppHandle,
    state: State<AppState>,
    accel: String,
) -> Result<AppData, String> {
    register_shortcut(&app, &accel)?;
    let mut config = state.config.lock().unwrap();
    config.shortcut = Some(accel);
    let cp = state.config_path.lock().unwrap();
    save_config(&cp, &config)?;
    Ok(build_app_data(&config))
}

// ------------------------- Tray + window -------------------------

pub fn run() {
    tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::Stdout,
                ))
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("naeasy".into()),
                    },
                ))
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        toggle_main(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            log::info!("naeasy starting up");
            // Pure menu-bar app: no Dock icon. Reach it via the menu-bar icon
            // or the global hotkey.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Resolve config path and load config into state.
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            let config_path = config_dir.join("config.json");
            let config = load_config(&config_path);
            let shortcut_accel = config.shortcut.clone().unwrap_or_else(default_shortcut);
            app.manage(AppState {
                config: Mutex::new(config),
                config_path: Mutex::new(config_path),
            });

            // Register the global hotkey that toggles the window.
            if let Err(e) = register_shortcut(app.handle(), &shortcut_accel) {
                log::warn!("global shortcut '{shortcut_accel}' not registered: {e}");
            }

            // Build the tray icon (menu-bar entry).
            use tauri::menu::{MenuBuilder, MenuItemBuilder};
            use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};

            let quit = MenuItemBuilder::with_id("quit", "Quit naeasy").build(app)?;
            let show = MenuItemBuilder::with_id("show", "Open naeasy").build(app)?;
            let menu = MenuBuilder::new(app).items(&[&show, &quit]).build()?;

            // Dedicated monochrome template icon for the menu bar — tints to
            // the light/dark menu bar automatically (like JetBrains Toolbox).
            // Fall back to the app icon if the tray PNG can't be decoded, so a
            // bad icon never crashes startup.
            let tray_builder = TrayIconBuilder::with_id("main-tray");
            let tray_builder = match tauri::image::Image::from_bytes(include_bytes!(
                "../icons/tray.png"
            )) {
                Ok(img) => tray_builder.icon(img).icon_as_template(true),
                Err(_) => tray_builder.icon(app.default_window_icon().unwrap().clone()),
            };

            let _tray = tray_builder
                .tooltip("naeasy — project navigator")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "quit" => app.exit(0),
                    "show" => show_main(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        rect,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let visible = window.is_visible().unwrap_or(false);
                            log::info!("tray clicked; window visible = {visible}");
                            if visible {
                                let _ = window.hide();
                            } else {
                                position_under_tray(&window, &rect);
                                let _ = window.show();
                                let _ = window.unminimize();
                                let _ = window.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Show the window on first launch so the app is visibly "there".
            log::info!("showing main window on launch");
            show_main(app.handle());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_data,
            rescan,
            add_workspace,
            remove_workspace,
            detect_ides,
            add_ide,
            remove_ide,
            set_default_ide,
            open_project,
            reveal_in_finder,
            quit_app,
            set_shortcut,
            scan_open_projects
        ])
        .on_window_event(|window, event| {
            // The red close button hides the window instead of quitting, so the
            // app keeps running. Reopen it from the Dock icon or menu-bar icon.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building naeasy")
        .run(|app_handle, event| {
            // Clicking the Dock icon when no window is visible reopens it.
            if let tauri::RunEvent::Reopen { .. } = event {
                show_main(app_handle);
            }
        });
}

/// Show, un-minimize and focus the main window.
fn show_main(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Quit the whole app (called from the UI "Quit" button).
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// Position the popover window horizontally centered under the tray icon,
/// clamped to the monitor so it can never end up off-screen.
fn position_under_tray(window: &tauri::WebviewWindow, rect: &tauri::Rect) {
    use tauri::{PhysicalPosition, Position};

    let win_w = match window.outer_size() {
        Ok(s) => s.width as f64,
        Err(_) => return,
    };
    let sf = window.scale_factor().unwrap_or(1.0);

    // Tray rect — convert to physical pixels.
    let tray_x = match rect.position {
        Position::Physical(p) => p.x as f64,
        Position::Logical(p) => p.x * sf,
    };
    let tray_w = match rect.size {
        tauri::Size::Physical(s) => s.width as f64,
        tauri::Size::Logical(s) => s.width * sf,
    };

    // Monitor bounds (physical). Fall back to centering if unknown.
    let (mon_x, mon_y, mon_w) = match window.primary_monitor() {
        Ok(Some(m)) => {
            let p = m.position();
            let s = m.size();
            (p.x as f64, p.y as f64, s.width as f64)
        }
        _ => {
            let _ = window.center();
            return;
        }
    };

    let mut x = tray_x + tray_w / 2.0 - win_w / 2.0;
    let min_x = mon_x + 8.0;
    let max_x = mon_x + mon_w - win_w - 8.0;
    if x < min_x {
        x = min_x;
    }
    if max_x > min_x && x > max_x {
        x = max_x;
    }
    // Just below the menu bar (~24pt).
    let y = mon_y + (26.0 * sf);

    let _ = window.set_position(Position::Physical(PhysicalPosition {
        x: x as i32,
        y: y as i32,
    }));
}
