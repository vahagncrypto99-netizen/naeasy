/// Registers the single global hotkey that toggles the main window.
pub struct ShortcutService;

impl ShortcutService {
    pub fn register(app: &tauri::AppHandle, accel: &str) -> Result<(), String> {
        use tauri_plugin_global_shortcut::GlobalShortcutExt;
        let shortcut: tauri_plugin_global_shortcut::Shortcut = accel
            .parse()
            .map_err(|_| format!("Invalid shortcut: {accel}"))?;
        let gs = app.global_shortcut();
        let _ = gs.unregister_all();
        gs.register(shortcut).map_err(|e| e.to_string())
    }
}
