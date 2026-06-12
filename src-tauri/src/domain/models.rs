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
    /// Candidate base branches for fast-start (editable, global).
    #[serde(default = "default_base_branches")]
    pub base_branches: Vec<String>,
    /// The globally selected base branch for fast-start.
    #[serde(default = "default_base_branch")]
    pub default_base_branch: String,
    /// Per-project base-branch override (path -> branch name).
    #[serde(default)]
    pub project_base_branches: std::collections::HashMap<String, String>,
    /// Floating window: draggable, position remembered. Off = pinned under
    /// the tray icon.
    #[serde(default)]
    pub window_float: bool,
    /// Remembered window position (physical px), float mode only.
    #[serde(default)]
    pub window_pos: Option<(i32, i32)>,
    /// Fixed-size window: not resizable, size taken from `fixed_size`.
    #[serde(default)]
    pub window_fixed: bool,
    /// Remembered window size (logical px), resizable mode.
    #[serde(default)]
    pub window_size: Option<(u32, u32)>,
    /// The size used in fixed mode.
    #[serde(default = "default_fixed_size")]
    pub fixed_size: (u32, u32),
    /// "Recent" section visibility and its max entries.
    #[serde(default = "default_true")]
    pub show_recent: bool,
    #[serde(default = "default_section_max")]
    pub max_recent: u32,
    /// "Pinned" section visibility and its max entries.
    #[serde(default = "default_true")]
    pub show_pinned: bool,
    #[serde(default = "default_section_max")]
    pub max_pinned: u32,
    /// Pinned project paths, in pin order.
    #[serde(default)]
    pub pinned: Vec<String>,
}

fn default_true() -> bool {
    true
}

pub fn default_base_branch() -> String {
    "master".to_string()
}

fn default_base_branches() -> Vec<String> {
    vec!["master".to_string()]
}

pub fn default_fixed_size() -> (u32, u32) {
    (380, 560)
}

fn default_section_max() -> u32 {
    3
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
            base_branches: default_base_branches(),
            default_base_branch: default_base_branch(),
            project_base_branches: Default::default(),
            window_float: false,
            window_pos: None,
            window_fixed: false,
            window_size: None,
            fixed_size: default_fixed_size(),
            show_recent: true,
            max_recent: default_section_max(),
            show_pinned: true,
            max_pinned: default_section_max(),
            pinned: Vec::new(),
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
    pub base_branches: Vec<String>,
    pub default_base_branch: String,
    pub project_base_branches: std::collections::HashMap<String, String>,
    pub window_float: bool,
    pub window_fixed: bool,
    pub fixed_size: (u32, u32),
    pub show_recent: bool,
    pub max_recent: u32,
    pub show_pinned: bool,
    pub max_pinned: u32,
    pub pinned: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shortcut_is_stable() {
        assert_eq!(default_shortcut(), "CmdOrCtrl+Shift+M");
    }
}
