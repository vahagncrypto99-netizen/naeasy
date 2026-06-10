use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ide {
    pub id: String,
    pub name: String,
    /// Full path to the application bundle, e.g. /Applications/PhpStorm.app
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub workspaces: Vec<Workspace>,
    #[serde(default)]
    pub ides: Vec<Ide>,
    #[serde(default)]
    pub default_ide_id: Option<String>,
    /// Global hotkey accelerator (e.g. "CmdOrCtrl+Shift+M"). None = default.
    #[serde(default)]
    pub shortcut: Option<String>,
    /// Last-opened info per project path.
    #[serde(default)]
    pub recents: std::collections::HashMap<String, Recent>,
    /// Preferred IDE per project path (path -> IDE id). Overrides the default
    /// IDE for that project.
    #[serde(default)]
    pub project_ides: std::collections::HashMap<String, String>,
    /// Open projects as tabs of one IDE window (merge windows) instead of a
    /// new window per project.
    #[serde(default = "default_true")]
    pub open_in_tabs: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Config {
    fn default() -> Self {
        Self {
            workspaces: Vec::new(),
            ides: Vec::new(),
            default_ide_id: None,
            shortcut: None,
            recents: Default::default(),
            project_ides: Default::default(),
            open_in_tabs: true,
        }
    }
}

pub fn default_shortcut() -> String {
    "CmdOrCtrl+Shift+M".to_string()
}

/// Per-project usage info, keyed by absolute project path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recent {
    /// Unix seconds of the last time the project was opened from naeasy.
    pub last_opened: u64,
    /// Name of the IDE it was last opened with.
    pub ide: String,
}

/// Unique-enough id for workspaces/IDEs: timestamp + process-wide counter.
pub fn gen_id() -> String {
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

pub fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// A node in the project tree. Leaves with `is_repo = true` are openable Git projects.
#[derive(Debug, Clone, Serialize)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    pub is_repo: bool,
    pub children: Vec<TreeNode>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceView {
    pub id: String,
    pub name: String,
    pub path: String,
    pub tree: TreeNode,
    pub repo_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppData {
    pub workspaces: Vec<WorkspaceView>,
    pub ides: Vec<Ide>,
    pub default_ide_id: Option<String>,
    pub shortcut: String,
    pub recents: std::collections::HashMap<String, Recent>,
    /// Preferred IDE per project path (path -> IDE id).
    pub project_ides: std::collections::HashMap<String, String>,
    pub open_in_tabs: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_is_stable() {
        assert_eq!(default_shortcut(), "CmdOrCtrl+Shift+M");
    }
}
