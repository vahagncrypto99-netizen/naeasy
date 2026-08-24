use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use super::workspace_service::WorkspaceService;
use crate::infra::keyboard_layouts::KeyboardLayoutProvider;

/// Tauri-managed application state: the service facade plus the keyboard-layout
/// provider used by layout-aware search.
pub struct AppState {
    pub service: WorkspaceService,
    pub layout_provider: Arc<dyn KeyboardLayoutProvider>,
    /// Set while a native dialog (the folder picker) owns the focus, so
    /// focus-loss auto-hide does not tuck the popover away behind it.
    /// macOS checks `keyWindow` instead and ignores this.
    pub autohide_suppressed: AtomicBool,
}
