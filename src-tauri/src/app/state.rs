use super::workspace_service::WorkspaceService;

/// Tauri-managed application state: just the service facade.
pub struct AppState {
    pub service: WorkspaceService,
}
