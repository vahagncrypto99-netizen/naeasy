use std::collections::HashMap;
use std::sync::Arc;

/// Strategy for OS-level project operations: launching a project in an IDE
/// (focus-or-open), scanning which projects are open in IDE windows, and
/// revealing a folder in the file manager. One implementation per OS,
/// selected once at the composition root.
///
/// `open` performs focus-or-open internally: if an IDE window for the project
/// already exists it is focused instead of launching a new instance.
/// With `in_tabs` the launcher additionally tries (best-effort) to keep the
/// projects in one window: native tabs where the OS has them, and otherwise
/// by attaching the project to the window the IDE already has open.
pub trait ProjectLauncher: Send + Sync {
    fn open(&self, app_path: &str, project_path: &str, in_tabs: bool) -> Result<(), String>;
    /// Which of the given project folder names are currently open in an IDE.
    /// Returns a map of name -> IDE process name. Best-effort.
    fn scan_open(&self, names: &[String]) -> HashMap<String, String>;
    fn reveal(&self, path: &str) -> Result<(), String>;
}

/// The launcher for the OS this binary was compiled for.
pub fn platform_launcher() -> Arc<dyn ProjectLauncher> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(MacLauncher)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Arc::new(LinuxLauncher)
    }
    #[cfg(target_os = "windows")]
    {
        Arc::new(WindowsLauncher)
    }
}

// ------------------------- macOS -------------------------

#[cfg(target_os = "macos")]
pub struct MacLauncher;

#[cfg(target_os = "macos")]
impl ProjectLauncher for MacLauncher {
    fn open(&self, app_path: &str, project_path: &str, in_tabs: bool) -> Result<(), String> {
        use std::process::Command;

        // 1. If the project is already open in this IDE, switch to its window
        //    (un-minimize + raise + focus) instead of opening it again.
        if focus_existing_window(app_path, project_path) {
            return Ok(());
        }

        // 2. Tab mode is enforced at the AppKit level: with the per-app
        //    AppleWindowTabbingMode=always default the IDE opens every new
        //    window as a native tab of the existing one — on whatever Space
        //    it lives, no Accessibility involved. Toggling off removes the
        //    override (back to the IDE's stock behavior).
        set_window_tabbing_mode(app_path, in_tabs);

        // 3. Launch / open. `open -a` also focuses an existing project
        //    window for IDEs that support it (JetBrains, VS Code, …).
        Command::new("open")
            .arg("-a")
            .arg(app_path)
            .arg(project_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch IDE: {e}"))?;

        // 3. Tab mode: once the new project window appears, merge the IDE's
        //    windows into native macOS tabs (best-effort, in the background —
        //    works for IDEs whose Window menu has a merge action, e.g.
        //    JetBrains; silently does nothing for the rest).
        if in_tabs {
            let proc_name = process_name_for(app_path);
            let project_name = std::path::Path::new(project_path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            if !project_name.is_empty() {
                std::thread::spawn(move || merge_windows_when_ready(&proc_name, &project_name));
            }
        }
        Ok(())
    }

    fn scan_open(&self, names: &[String]) -> HashMap<String, String> {
        scan_open_macos(names)
    }

    fn reveal(&self, path: &str) -> Result<(), String> {
        use std::process::Command;

        Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Enforce (or lift) the per-app "new windows open as native tabs" default.
/// `defaults write <bundle-id> AppleWindowTabbingMode always` is what e.g.
/// VS Code's `window.nativeTabs` does under the hood; JetBrains IDEs honor
/// native tabbing since 2024.2. Works across Spaces because AppKit attaches
/// the tab in-process, no window scripting involved.
#[cfg(target_os = "macos")]
fn set_window_tabbing_mode(app_path: &str, in_tabs: bool) {
    use std::process::Command;

    let info_plist = format!("{}/Contents/Info", app_path.trim_end_matches('/'));
    let bundle_id = match Command::new("defaults")
        .args(["read", &info_plist, "CFBundleIdentifier"])
        .output()
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => return, // not a bundle (CLI editor etc.) — nothing to set
    };
    if bundle_id.is_empty() {
        return;
    }

    let result = if in_tabs {
        Command::new("defaults")
            .args(["write", &bundle_id, "AppleWindowTabbingMode", "-string", "always"])
            .status()
    } else {
        // Removing a missing key fails — that's fine, treat as success.
        let _ = Command::new("defaults")
            .args(["delete", &bundle_id, "AppleWindowTabbingMode"])
            .status();
        return;
    };
    match result {
        Ok(s) if s.success() => {
            log::info!("tabbing mode 'always' set for {bundle_id}");
        }
        other => log::warn!("failed to set tabbing mode for {bundle_id}: {other:?}"),
    }
}

/// Map an app bundle name to the process name used by System Events.
/// Most apps match their bundle name; a few editors differ.
#[cfg(target_os = "macos")]
fn process_name_for(app_path: &str) -> String {
    let bundle = super::ide_detector::ide_name_from_path(app_path);
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
    use std::path::Path;

    let proc_name = process_name_for(app_path);
    let project_name = Path::new(project_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    if project_name.is_empty() {
        return false;
    }
    focus_window_of(&proc_name, &project_name)
}

/// Raise + focus the window (or native tab) of `proc_name` whose title
/// contains `project_name`.
#[cfg(target_os = "macos")]
fn focus_window_of(proc_name: &str, project_name: &str) -> bool {
    use std::process::Command;

    let app_q = applescript_quote(proc_name);
    let proj_q = applescript_quote(project_name);

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

/// Poll the IDE for up to ~12s; as soon as the freshly opened project's
/// window appears, click the merge action in its Window menu so projects
/// become native macOS tabs, then raise the project's tab.
///
/// Waiting for the PROJECT window (not "window count > 1") matters: the
/// Accessibility API only sees windows on the active Space, so an IDE window
/// living on another (e.g. fullscreen) Space is invisible to a count check —
/// while the new window always opens on the current Space. The IDE-level
/// merge action then collects windows across Spaces.
///
/// JetBrains IDEs name it "Merge All Project Windows" (2024.2+); standard
/// AppKit apps (incl. VS Code with `window.nativeTabs`) — "Merge All Windows".
/// Best-effort: needs Accessibility (already required for focus/scan); a
/// silent no-op for IDEs that have neither menu item.
#[cfg(target_os = "macos")]
fn merge_windows_when_ready(proc_name: &str, project_name: &str) {
    use std::process::Command;
    use std::thread::sleep;
    use std::time::Duration;

    let app_q = applescript_quote(proc_name);
    let proj_q = applescript_quote(project_name);
    let script = format!(
        r#"tell application "System Events"
    if not (exists (process {app})) then return "SKIP"
    tell process {app}
        if not (exists (window whose name contains {proj})) then return "WAIT"
        try
            click menu item "Merge All Project Windows" of menu "Window" of menu bar item "Window" of menu bar 1
            return "MERGED"
        end try
        try
            click menu item "Merge All Windows" of menu "Window" of menu bar item "Window" of menu bar 1
            return "MERGED"
        end try
    end tell
end tell
return "SKIP""#,
        app = app_q,
        proj = proj_q
    );

    for attempt in 1..=6 {
        sleep(Duration::from_millis(2000));
        match Command::new("osascript").arg("-e").arg(&script).output() {
            Ok(out) => {
                let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                log::info!("merge[{attempt}] {proc_name}/{project_name}: {stdout} {stderr}");
                if stdout.contains("MERGED") {
                    // Bring the merged tab forward — the tab group may live
                    // on another Space (e.g. the fullscreen IDE window).
                    sleep(Duration::from_millis(700));
                    focus_window_of(proc_name, project_name);
                    return;
                }
                if stdout.contains("WAIT") {
                    continue;
                }
                return;
            }
            Err(e) => {
                log::warn!("merge[{attempt}] {proc_name}: osascript failed: {e}");
                return;
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn scan_open_macos(names: &[String]) -> HashMap<String, String> {
    use std::process::Command;

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

// ------------------------- Linux (and other unix) -------------------------

/// The flag that makes an IDE add a project to the window it already has
/// open, if it has such a flag.
///
/// macOS gets real window tabs from AppKit; Linux has no equivalent, so tab
/// mode means "one window, several projects in it". Only VS Code and its
/// forks expose that on the command line, as `--add` — the multi-root flag,
/// not `--reuse-window`, which drops the folder that is open and takes its
/// place. JetBrains keeps the same choice in a setting of its own (see
/// `set_open_project_mode`), and Zed/Sublime have nothing comparable.
#[cfg(all(unix, not(target_os = "macos")))]
fn attach_project_flag(app_path: &str) -> Option<&'static str> {
    const VSCODE_FAMILY: &[&str] = &["code", "code-insiders", "codium", "cursor", "windsurf"];

    let stem = std::path::Path::new(app_path)
        .file_stem()?
        .to_string_lossy()
        .to_lowercase();
    VSCODE_FAMILY.contains(&stem.as_str()).then_some("--add")
}

/// The config-directory names a JetBrains launcher writes its settings
/// under, e.g. `/snap/bin/phpstorm` → `PhpStorm`. `None` for every other
/// editor — nobody else keeps this preference in a file we can set.
#[cfg(all(unix, not(target_os = "macos")))]
fn jetbrains_config_prefixes(app_path: &str) -> Option<&'static [&'static str]> {
    // The command names are the ones the detector already looks for; IDEA is
    // the one product whose config directory is not simply its command name
    // (Ultimate writes IntelliJIdea<version>, Community IdeaIC<version>).
    const PRODUCTS: &[(&str, &[&str])] = &[
        ("phpstorm", &["PhpStorm"]),
        ("goland", &["GoLand"]),
        ("datagrip", &["DataGrip"]),
        ("pycharm", &["PyCharm"]),
        ("idea", &["IntelliJIdea", "IdeaIC"]),
        ("webstorm", &["WebStorm"]),
        ("clion", &["CLion"]),
        ("rubymine", &["RubyMine"]),
        ("rider", &["Rider"]),
        ("rustrover", &["RustRover"]),
    ];

    let stem = std::path::Path::new(app_path)
        .file_stem()?
        .to_string_lossy()
        .to_lowercase();
    PRODUCTS
        .iter()
        .find(|(cmd, _)| *cmd == stem)
        .map(|(_, dirs)| *dirs)
}

/// Where JetBrains stores the "Open project in" preference, and the two
/// values naeasy sets it to. The third one, `1` ("the current window"), is
/// deliberately not among them: it does not add the project to the open
/// window, it closes the project that is there and takes its place.
#[cfg(all(unix, not(target_os = "macos")))]
const OPEN_PROJECT_OPTION: &str = "confirmOpenNewProject2";
#[cfg(all(unix, not(target_os = "macos")))]
const OPEN_PROJECT_ATTACH: i32 = 2;
#[cfg(all(unix, not(target_os = "macos")))]
const OPEN_PROJECT_NEW_WINDOW: i32 = 0;

/// Point JetBrains' "Open project in" at the mode tab mode needs.
///
/// The Linux counterpart of the macOS `AppleWindowTabbingMode` write. There
/// are no project tabs on Linux at all — JetBrains tracks that as an open
/// feature request (IJPL-43816), and its own docs put "Merge All Project
/// Windows" under macOS only — so the nearest an IDE gets is *attaching*:
/// the project joins the window that is already open as another root of its
/// Project tool window, and the project already in that window stays open.
/// Off, every project gets a window of its own and naeasy switches between
/// them. The preference lives in the IDE's own config file, and a running
/// IDE holds it in memory, so a change made here takes hold the next time
/// that IDE starts.
#[cfg(all(unix, not(target_os = "macos")))]
fn set_open_project_mode(app_path: &str, in_tabs: bool) {
    let Some(prefixes) = jetbrains_config_prefixes(app_path) else {
        return;
    };
    let Some(dir) = newest_jetbrains_config_dir(prefixes) else {
        return;
    };
    let mode = if in_tabs {
        OPEN_PROJECT_ATTACH
    } else {
        OPEN_PROJECT_NEW_WINDOW
    };
    let file = dir.join("options").join("ide.general.xml");
    let xml = std::fs::read_to_string(&file).unwrap_or_default();
    let Some(patched) = patched_general_settings(&xml, mode) else {
        return; // already set that way, or a document we don't recognise
    };
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(&file, patched) {
        Ok(()) => log::info!(
            "'Open project in' -> {} in {}",
            if in_tabs {
                "attach to the open window"
            } else {
                "a new window"
            },
            file.display()
        ),
        Err(e) => log::warn!("could not update {}: {e}", file.display()),
    }
}

/// The config directory of the newest installed version of a JetBrains
/// product, e.g. `~/.config/JetBrains/PhpStorm2026.2`.
#[cfg(all(unix, not(target_os = "macos")))]
fn newest_jetbrains_config_dir(prefixes: &[&str]) -> Option<std::path::PathBuf> {
    let root = std::path::PathBuf::from(std::env::var_os("HOME")?)
        .join(".config")
        .join("JetBrains");
    let mut versions: Vec<String> = std::fs::read_dir(&root)
        .ok()?
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .filter(|name| prefixes.iter().any(|p| is_version_dir(name, p)))
        .collect();
    versions.sort();
    versions.pop().map(|name| root.join(name))
}

/// `PhpStorm2026.2` is an installed version; `PhpStorm2025.3-backup` is a
/// copy someone left behind, and settings written there reach no IDE.
#[cfg(all(unix, not(target_os = "macos")))]
fn is_version_dir(name: &str, prefix: &str) -> bool {
    match name.strip_prefix(prefix) {
        Some(rest) => !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '.'),
        None => false,
    }
}

/// Rewrite an `ide.general.xml` document so the "Open project in" option
/// holds `value`. `None` back means there is nothing to write: the file
/// already says exactly that, or it is a document this doesn't recognise
/// (blindly overwriting somebody's IDE settings is not worth a tab mode).
/// Pure — no filesystem — so it is unit-testable.
#[cfg(all(unix, not(target_os = "macos")))]
fn patched_general_settings(xml: &str, value: i32) -> Option<String> {
    // JetBrains writes one element per line, so lines are the natural unit.
    let indent_of = |line: &str| line[..line.len() - line.trim_start().len()].to_string();
    let element = |indent: &str, v: i32| {
        format!("{indent}<option name=\"{OPEN_PROJECT_OPTION}\" value=\"{v}\" />")
    };
    let joined = |lines: Vec<String>| lines.join("\n") + "\n";

    let mut lines: Vec<String> = xml.lines().map(str::to_string).collect();

    // The option is already in the file — rewrite that line.
    if let Some(i) = lines.iter().position(|l| l.contains(OPEN_PROJECT_OPTION)) {
        let line = element(&indent_of(&lines[i]), value);
        if lines[i] == line {
            return None;
        }
        lines[i] = line;
        return Some(joined(lines));
    }

    // The component is there — the option becomes its first child.
    if let Some(i) = lines
        .iter()
        .position(|l| l.contains("<component name=\"GeneralSettings\""))
    {
        let indent = indent_of(&lines[i]);
        if lines[i].trim_end().ends_with("/>") {
            // An empty <component … /> — give it a body to hold the option.
            lines[i] = format!("{indent}<component name=\"GeneralSettings\">");
            lines.insert(i + 1, element(&format!("{indent}  "), value));
            lines.insert(i + 2, format!("{indent}</component>"));
        } else {
            lines.insert(i + 1, element(&format!("{indent}  "), value));
        }
        return Some(joined(lines));
    }

    // A settings document without that component — add the whole block.
    if let Some(i) = lines.iter().position(|l| l.contains("<application")) {
        lines.insert(i + 1, "  <component name=\"GeneralSettings\">".to_string());
        lines.insert(i + 2, element("    ", value));
        lines.insert(i + 3, "  </component>".to_string());
        return Some(joined(lines));
    }

    // No file yet — write a fresh one. Anything else is left alone.
    xml.trim().is_empty().then(|| {
        format!(
            "<application>\n  <component name=\"GeneralSettings\">\n{}\n  </component>\n</application>\n",
            element("    ", value)
        )
    })
}

#[cfg(all(unix, not(target_os = "macos")))]
pub struct LinuxLauncher;

#[cfg(all(unix, not(target_os = "macos")))]
impl ProjectLauncher for LinuxLauncher {
    fn open(&self, app_path: &str, project_path: &str, in_tabs: bool) -> Result<(), String> {
        use std::process::Command;

        // Try to focus an already-open window (via wmctrl) before launching.
        if focus_existing_window(app_path, project_path) {
            return Ok(());
        }
        // No native window tabs here. Tab mode gets as close as each IDE
        // can: JetBrains attaches the project to the window that is already
        // open, VS Code adds it to that window as another root. Either way
        // what is open stays open. JetBrains reads it from its own settings,
        // VS Code from a flag.
        set_open_project_mode(app_path, in_tabs);

        let mut command = Command::new(app_path);
        if in_tabs {
            if let Some(flag) = attach_project_flag(app_path) {
                command.arg(flag);
            }
        }
        command
            .arg(project_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch IDE: {e}"))
    }

    fn scan_open(&self, names: &[String]) -> HashMap<String, String> {
        #[cfg(target_os = "linux")]
        {
            scan_open_linux(names)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = names;
            HashMap::new()
        }
    }

    fn reveal(&self, path: &str) -> Result<(), String> {
        let _ = path;
        Err("Reveal in Finder is only supported on macOS".into())
    }
}

/// Linux (X11): focus a window whose title contains the project folder name,
/// using `wmctrl`. Returns false if wmctrl is missing or nothing matched.
#[cfg(all(unix, not(target_os = "macos")))]
fn focus_existing_window(_app_path: &str, project_path: &str) -> bool {
    use std::path::Path;
    use std::process::Command;

    let name = match Path::new(project_path).file_name() {
        Some(n) => n.to_string_lossy().to_string(),
        None => return false,
    };
    if name.is_empty() {
        return false;
    }
    let out = match Command::new("wmctrl").arg("-l").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return false,
    };
    for line in out.lines() {
        // Format: <winid> <desktop> <host> <title...>
        let mut it = line.split_whitespace();
        let id = it.next();
        let _desktop = it.next();
        let _host = it.next();
        let title = it.collect::<Vec<_>>().join(" ");
        if title.contains(&name) {
            if let Some(id) = id {
                let _ = Command::new("wmctrl").arg("-i").arg("-a").arg(id).status();
                return true;
            }
        }
    }
    false
}

/// Linux (X11): match project names against open IDE window titles via wmctrl.
#[cfg(target_os = "linux")]
fn scan_open_linux(names: &[String]) -> HashMap<String, String> {
    use std::process::Command;

    let mut result = HashMap::new();
    // `wmctrl -lx`: <winid> <desktop> <wm_class> <host> <title...>
    let out = match Command::new("wmctrl").arg("-lx").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return result,
    };
    let lines: Vec<(String, String)> = out
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let _id = it.next()?;
            let _desktop = it.next()?;
            let class = it.next()?.to_string();
            let _host = it.next()?;
            let title = it.collect::<Vec<_>>().join(" ");
            Some((class, title))
        })
        .collect();

    for name in names {
        if name.is_empty() {
            continue;
        }
        for (class, title) in &lines {
            if title.contains(name.as_str()) {
                result.insert(name.clone(), ide_label_from_class(class));
                break;
            }
        }
    }
    result
}

#[cfg(target_os = "linux")]
fn ide_label_from_class(class: &str) -> String {
    let c = class.to_lowercase();
    let map = [
        ("phpstorm", "PhpStorm"),
        ("goland", "GoLand"),
        ("datagrip", "DataGrip"),
        ("pycharm", "PyCharm"),
        ("idea", "IntelliJ IDEA"),
        ("webstorm", "WebStorm"),
        ("clion", "CLion"),
        ("rubymine", "RubyMine"),
        ("rider", "Rider"),
        ("rustrover", "RustRover"),
        ("cursor", "Cursor"),
        ("code", "VS Code"),
        ("zed", "Zed"),
    ];
    for (k, v) in map {
        if c.contains(k) {
            return v.to_string();
        }
    }
    "IDE".to_string()
}

// ------------------------- Windows -------------------------

#[cfg(target_os = "windows")]
pub struct WindowsLauncher;

#[cfg(target_os = "windows")]
impl ProjectLauncher for WindowsLauncher {
    fn open(&self, app_path: &str, project_path: &str, in_tabs: bool) -> Result<(), String> {
        use std::process::Command;

        // Tab mode is macOS-only (native window tabs); ignored here.
        let _ = in_tabs;
        Command::new(app_path)
            .arg(project_path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch IDE: {e}"))
    }

    fn scan_open(&self, names: &[String]) -> HashMap<String, String> {
        let _ = names;
        HashMap::new()
    }

    fn reveal(&self, path: &str) -> Result<(), String> {
        let _ = path;
        Err("Reveal in Finder is only supported on macOS".into())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    #[test]
    fn process_name_maps_vscode() {
        use super::process_name_for;

        assert_eq!(
            process_name_for("/Applications/Visual Studio Code.app"),
            "Code"
        );
        assert_eq!(process_name_for("/Applications/PhpStorm.app"), "PhpStorm");
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn recognises_the_jetbrains_config_directories() {
        use super::{is_version_dir, jetbrains_config_prefixes};

        assert_eq!(jetbrains_config_prefixes("/snap/bin/phpstorm"), Some(&["PhpStorm"][..]));
        assert_eq!(
            jetbrains_config_prefixes("/home/u/.local/share/JetBrains/Toolbox/scripts/idea"),
            Some(&["IntelliJIdea", "IdeaIC"][..])
        );
        assert_eq!(jetbrains_config_prefixes("/usr/bin/code"), None);

        assert!(is_version_dir("PhpStorm2026.2", "PhpStorm"));
        // A leftover copy holds settings no IDE ever reads.
        assert!(!is_version_dir("PhpStorm2025.3-backup", "PhpStorm"));
        assert!(!is_version_dir("PhpStorm", "PhpStorm"));
        assert!(!is_version_dir("GoLand2026.2", "PhpStorm"));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn attaches_new_projects_to_the_open_window() {
        use super::{patched_general_settings, OPEN_PROJECT_ATTACH, OPEN_PROJECT_NEW_WINDOW};

        // The option is there, set to "new window" (0) — rewrite that line
        // and leave everything around it exactly as it was.
        let xml = "<application>\n  <component name=\"GeneralSettings\">\n    <option name=\"confirmExit\" value=\"false\" />\n    <option name=\"confirmOpenNewProject2\" value=\"0\" />\n  </component>\n</application>\n";
        let out = patched_general_settings(xml, OPEN_PROJECT_ATTACH).unwrap();
        assert!(out.contains("<option name=\"confirmOpenNewProject2\" value=\"2\" />"));
        assert!(out.contains("<option name=\"confirmExit\" value=\"false\" />"));
        // 1 is "the current window", and that one closes the project that is
        // open to take its place. Tab mode must never leave it behind.
        assert!(!out.contains("value=\"1\""));

        // Already set that way — nothing to write.
        assert!(patched_general_settings(&out, OPEN_PROJECT_ATTACH).is_none());

        // Off, every project gets a window of its own again.
        let off = patched_general_settings(&out, OPEN_PROJECT_NEW_WINDOW).unwrap();
        assert!(off.contains("<option name=\"confirmOpenNewProject2\" value=\"0\" />"));
        assert!(off.contains("confirmExit"));
        assert!(patched_general_settings(&off, OPEN_PROJECT_NEW_WINDOW).is_none());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn adds_the_option_to_documents_that_lack_it() {
        use super::{patched_general_settings, OPEN_PROJECT_ATTACH};

        // No file yet.
        let fresh = patched_general_settings("", OPEN_PROJECT_ATTACH).unwrap();
        assert!(fresh.contains("<component name=\"GeneralSettings\">"));
        assert!(fresh.contains("<option name=\"confirmOpenNewProject2\" value=\"2\" />"));

        // A settings document with other components but not this one.
        let other = "<application>\n  <component name=\"Registry\">\n  </component>\n</application>\n";
        let out = patched_general_settings(other, OPEN_PROJECT_ATTACH).unwrap();
        assert!(out.contains("Registry"));
        assert!(out.contains("confirmOpenNewProject2"));

        // An empty component gets a body to hold the option.
        let empty = "<application>\n  <component name=\"GeneralSettings\" />\n</application>\n";
        let out = patched_general_settings(empty, OPEN_PROJECT_ATTACH).unwrap();
        assert!(out.contains("<component name=\"GeneralSettings\">"));
        assert!(out.contains("</component>"));
        assert!(out.contains("confirmOpenNewProject2"));

        // Something we don't recognise is left alone rather than overwritten.
        assert!(patched_general_settings("not xml at all", OPEN_PROJECT_ATTACH).is_none());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn attaches_to_the_open_window_only_where_the_ide_supports_it() {
        use super::attach_project_flag;

        assert_eq!(attach_project_flag("/usr/bin/cursor"), Some("--add"));
        assert_eq!(attach_project_flag("/usr/bin/code"), Some("--add"));
        assert_eq!(attach_project_flag("/usr/bin/codium"), Some("--add"));
        // JetBrains keeps this in its own settings, Zed has no such flag.
        assert_eq!(attach_project_flag("/snap/bin/phpstorm"), None);
        assert_eq!(attach_project_flag("/usr/bin/zed"), None);
    }
}
