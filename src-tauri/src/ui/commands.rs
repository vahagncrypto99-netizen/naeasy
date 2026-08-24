//! `#[tauri::command]` handlers — thin adapters that translate IPC calls into
//! service calls. No business logic here.

use tauri::State;

use crate::app::state::AppState;
use crate::domain::models::AppData;
use crate::infra::keyboard_layouts::LayoutMap;
use crate::infra::shortcut::ShortcutService;

#[tauri::command]
pub fn get_data(state: State<AppState>) -> AppData {
    state.service.app_data()
}

/// Installed keyboard-layout maps for layout-aware search. Built fresh from the
/// OS (empty on platforms without a provider).
#[tauri::command]
pub fn get_layout_maps(state: State<AppState>) -> Vec<LayoutMap> {
    state.layout_provider.layouts()
}

#[tauri::command]
pub fn rescan(state: State<AppState>) -> AppData {
    state.service.app_data()
}

#[tauri::command]
pub fn add_workspace(state: State<AppState>, path: String) -> Result<AppData, String> {
    state.service.add_workspace(path)
}

#[tauri::command]
pub fn remove_workspace(state: State<AppState>, id: String) -> Result<AppData, String> {
    state.service.remove_workspace(&id)
}

#[tauri::command]
pub fn detect_ides(state: State<AppState>) -> Result<AppData, String> {
    state.service.detect_ides()
}

#[tauri::command]
pub fn add_ide(state: State<AppState>, path: String) -> Result<AppData, String> {
    state.service.add_ide(path)
}

#[tauri::command]
pub fn remove_ide(state: State<AppState>, id: String) -> Result<AppData, String> {
    state.service.remove_ide(&id)
}

#[tauri::command]
pub fn set_default_ide(state: State<AppState>, id: String) -> Result<AppData, String> {
    state.service.set_default_ide(id)
}

/// Open a project folder in the chosen (or default) IDE.
#[tauri::command]
pub fn open_project(
    state: State<AppState>,
    project_path: String,
    ide_id: Option<String>,
) -> Result<(), String> {
    state.service.open_project(project_path, ide_id)
}

/// Reveal the project folder in Finder.
#[tauri::command]
pub fn reveal_in_finder(state: State<AppState>, path: String) -> Result<(), String> {
    state.service.reveal(&path)
}

/// Which of the given project folder names are currently open in an IDE.
/// Returns a map of name -> IDE process name. Best-effort (needs Accessibility).
#[tauri::command]
pub fn scan_open_projects(
    state: State<AppState>,
    names: Vec<String>,
) -> std::collections::HashMap<String, String> {
    state.service.scan_open(&names)
}

/// Remember (or clear, with null) the preferred IDE for one project.
#[tauri::command]
pub fn set_project_ide(
    state: State<AppState>,
    project_path: String,
    ide_id: Option<String>,
) -> Result<AppData, String> {
    state.service.set_project_ide(project_path, ide_id)
}

/// Toggle tabs-vs-windows mode for opening projects.
#[tauri::command]
pub fn set_open_in_tabs(state: State<AppState>, enabled: bool) -> Result<AppData, String> {
    state.service.set_open_in_tabs(enabled)
}

/// Open the project's git remote (origin) in the browser.
#[tauri::command]
pub fn open_repo_url(state: State<AppState>, path: String) -> Result<(), String> {
    let url = state.service.repo_web_url(&path)?;
    tauri_plugin_opener::open_url(&url, None::<String>).map_err(|e| e.to_string())
}

/// Create (or reuse) a branch from the chosen base and open the project.
#[tauri::command]
pub fn fast_start(
    state: State<AppState>,
    project_path: String,
    branch: String,
    base: Option<String>,
) -> Result<(), String> {
    state.service.fast_start(project_path, branch, base)
}

#[tauri::command]
pub fn add_base_branch(state: State<AppState>, name: String) -> Result<AppData, String> {
    state.service.add_base_branch(name)
}

#[tauri::command]
pub fn remove_base_branch(state: State<AppState>, name: String) -> Result<AppData, String> {
    state.service.remove_base_branch(&name)
}

#[tauri::command]
pub fn set_default_base_branch(state: State<AppState>, name: String) -> Result<AppData, String> {
    state.service.set_default_base_branch(name)
}

#[tauri::command]
pub fn set_shortcut(
    app: tauri::AppHandle,
    state: State<AppState>,
    accel: String,
) -> Result<AppData, String> {
    ShortcutService::register(&app, &accel)?;
    state.service.set_shortcut(accel)
}

/// Open the MR/PR list for the project's current branch.
#[tauri::command]
pub fn open_last_mr(state: State<AppState>, path: String) -> Result<(), String> {
    let url = state.service.last_mr_url(&path)?;
    tauri_plugin_opener::open_url(&url, None::<String>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn toggle_pin(state: State<AppState>, path: String) -> Result<AppData, String> {
    state.service.toggle_pin(path)
}

#[tauri::command]
pub fn set_section_prefs(
    state: State<AppState>,
    show_recent: Option<bool>,
    max_recent: Option<u32>,
    show_pinned: Option<bool>,
    max_pinned: Option<u32>,
) -> Result<AppData, String> {
    state
        .service
        .set_section_prefs(show_recent, max_recent, show_pinned, max_pinned)
}

/// Window mode prefs; geometry is applied to the live window immediately.
#[tauri::command]
pub fn set_window_prefs(
    app: tauri::AppHandle,
    state: State<AppState>,
    float: Option<bool>,
    fixed: Option<bool>,
    fixed_size: Option<(u32, u32)>,
) -> Result<AppData, String> {
    let data = state.service.set_window_prefs(float, fixed, fixed_size)?;
    super::tray::apply_window_geometry(&app);
    Ok(data)
}

/// Hide the popover (used by the frontend after opening a project) — goes
/// through the single hide path so transient UI gets reset without flashes.
#[tauri::command]
pub fn hide_window(app: tauri::AppHandle) {
    super::tray::hide_main(&app);
}

/// Hold off focus-loss auto-hide while the frontend has a native dialog
/// open (folder picker) — that dialog takes the focus, and without this the
/// popover would hide behind it mid-pick.
#[tauri::command]
pub fn set_autohide_suppressed(state: State<AppState>, suppressed: bool) {
    state
        .autohide_suppressed
        .store(suppressed, std::sync::atomic::Ordering::Relaxed);
}

/// Quit the whole app (called from the UI "Quit" button).
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
