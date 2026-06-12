//! `#[tauri::command]` handlers — thin adapters that translate IPC calls into
//! service calls. No business logic here.

use tauri::State;

use crate::app::state::AppState;
use crate::domain::models::AppData;
use crate::infra::shortcut::ShortcutService;

#[tauri::command]
pub fn get_data(state: State<AppState>) -> AppData {
    state.service.app_data()
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

/// Quit the whole app (called from the UI "Quit" button).
#[tauri::command]
pub fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
