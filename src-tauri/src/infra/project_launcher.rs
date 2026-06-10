use std::collections::HashMap;
use std::sync::Arc;

/// Strategy for OS-level project operations: launching a project in an IDE
/// (focus-or-open), scanning which projects are open in IDE windows, and
/// revealing a folder in the file manager. One implementation per OS,
/// selected once at the composition root.
///
/// `open` performs focus-or-open internally: if an IDE window for the project
/// already exists it is focused instead of launching a new instance.
pub trait ProjectLauncher: Send + Sync {
    fn open(&self, app_path: &str, project_path: &str) -> Result<(), String>;
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
    fn open(&self, app_path: &str, project_path: &str) -> Result<(), String> {
        use std::process::Command;

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
    use std::process::Command;

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

#[cfg(all(unix, not(target_os = "macos")))]
pub struct LinuxLauncher;

#[cfg(all(unix, not(target_os = "macos")))]
impl ProjectLauncher for LinuxLauncher {
    fn open(&self, app_path: &str, project_path: &str) -> Result<(), String> {
        use std::process::Command;

        // Try to focus an already-open window (via wmctrl) before launching.
        if focus_existing_window(app_path, project_path) {
            return Ok(());
        }
        Command::new(app_path)
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
    fn open(&self, app_path: &str, project_path: &str) -> Result<(), String> {
        use std::process::Command;

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
}
