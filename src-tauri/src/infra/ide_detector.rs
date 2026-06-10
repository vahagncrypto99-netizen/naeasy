use std::path::Path;
use std::sync::Arc;

use crate::domain::models::Ide;
use crate::gen_id;

/// Strategy for auto-detecting installed IDEs. One implementation per OS,
/// selected once at the composition root.
pub trait IdeDetector: Send + Sync {
    /// Detected IDEs, sorted by name.
    fn detect(&self) -> Vec<Ide>;
}

/// The detector for the OS this binary was compiled for.
pub fn platform_detector() -> Arc<dyn IdeDetector> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(MacIdeDetector)
    }
    #[cfg(target_os = "linux")]
    {
        Arc::new(LinuxIdeDetector)
    }
    #[cfg(target_os = "windows")]
    {
        Arc::new(WindowsIdeDetector)
    }
}

pub(crate) fn ide_name_from_path(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn sort_by_name(mut ides: Vec<Ide>) -> Vec<Ide> {
    ides.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    ides
}

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from)
}

// ---- macOS: scan /Applications for known *.app bundles ----

#[cfg(target_os = "macos")]
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

#[cfg(target_os = "macos")]
pub struct MacIdeDetector;

#[cfg(target_os = "macos")]
impl IdeDetector for MacIdeDetector {
    fn detect(&self) -> Vec<Ide> {
        let mut found: Vec<Ide> = Vec::new();
        let mut dirs: Vec<std::path::PathBuf> = vec![std::path::PathBuf::from("/Applications")];
        if let Some(home) = home_dir() {
            dirs.push(home.join("Applications"));
            dirs.push(home.join("Applications/JetBrains Toolbox"));
        }
        for dir in dirs {
            scan_apps_dir(&dir, &mut found);
        }
        sort_by_name(found)
    }
}

#[cfg(target_os = "macos")]
fn scan_apps_dir(dir: &Path, found: &mut Vec<Ide>) {
    let entries = match std::fs::read_dir(dir) {
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

// ---- Linux: search PATH + JetBrains Toolbox scripts ----

/// command name -> display name (Linux PATH / Toolbox scripts).
#[cfg(target_os = "linux")]
const LINUX_CMDS: &[(&str, &str)] = &[
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
    ("code", "VS Code"),
    ("codium", "VSCodium"),
    ("cursor", "Cursor"),
    ("zed", "Zed"),
    ("subl", "Sublime Text"),
];

#[cfg(target_os = "linux")]
pub struct LinuxIdeDetector;

#[cfg(target_os = "linux")]
impl IdeDetector for LinuxIdeDetector {
    fn detect(&self) -> Vec<Ide> {
        use std::path::PathBuf;

        let mut found: Vec<Ide> = Vec::new();

        let path_var = std::env::var("PATH").unwrap_or_default();
        let path_dirs: Vec<PathBuf> = std::env::split_paths(&path_var).collect();
        for (cmd, name) in LINUX_CMDS {
            for dir in &path_dirs {
                let p = dir.join(cmd);
                if p.is_file() {
                    let path_str = p.to_string_lossy().to_string();
                    if !found.iter().any(|i| i.path == path_str) {
                        found.push(Ide {
                            id: gen_id(),
                            name: name.to_string(),
                            path: path_str,
                        });
                    }
                    break;
                }
            }
        }

        if let Some(home) = home_dir() {
            let scripts = home.join(".local/share/JetBrains/Toolbox/scripts");
            if let Ok(entries) = std::fs::read_dir(&scripts) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if !p.is_file() {
                        continue;
                    }
                    let stem = p
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    if let Some((_, name)) =
                        LINUX_CMDS.iter().find(|(c, _)| stem.eq_ignore_ascii_case(c))
                    {
                        let path_str = p.to_string_lossy().to_string();
                        if !found.iter().any(|i| i.path == path_str) {
                            found.push(Ide {
                                id: gen_id(),
                                name: name.to_string(),
                                path: path_str,
                            });
                        }
                    }
                }
            }
        }
        sort_by_name(found)
    }
}

// ---- Windows: common install locations (rest via manual Add IDE) ----

#[cfg(target_os = "windows")]
pub struct WindowsIdeDetector;

#[cfg(target_os = "windows")]
impl IdeDetector for WindowsIdeDetector {
    fn detect(&self) -> Vec<Ide> {
        use std::path::PathBuf;

        let mut found: Vec<Ide> = Vec::new();
        if let Ok(lad) = std::env::var("LOCALAPPDATA") {
            for (rel, name) in [
                ("Programs\\Microsoft VS Code\\Code.exe", "VS Code"),
                ("Programs\\cursor\\Cursor.exe", "Cursor"),
            ] {
                let p = PathBuf::from(&lad).join(rel);
                if p.is_file() {
                    found.push(Ide {
                        id: gen_id(),
                        name: name.to_string(),
                        path: p.to_string_lossy().to_string(),
                    });
                }
            }
        }
        sort_by_name(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ide_name_from_path_extracts_stem() {
        assert_eq!(ide_name_from_path("/Applications/PhpStorm.app"), "PhpStorm");
        assert_eq!(
            ide_name_from_path("/Users/x/Applications/GoLand.app"),
            "GoLand"
        );
    }
}
