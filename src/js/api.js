// The only module that talks to the Tauri API (withGlobalTauri = true).
// Everything else imports these wrappers.

const { invoke } = window.__TAURI__.core;
const { open: openDialog } = window.__TAURI__.dialog;
const autostartPlugin = window.__TAURI__.autostart;
const getCurrentWindow = window.__TAURI__.window?.getCurrentWindow;

export const getData = () => invoke("get_data");
export const getLayoutMaps = () => invoke("get_layout_maps");
export const rescan = () => invoke("rescan");
export const addWorkspace = (path) => invoke("add_workspace", { path });
export const removeWorkspace = (id) => invoke("remove_workspace", { id });
export const detectIdes = () => invoke("detect_ides");
export const addIde = (path) => invoke("add_ide", { path });
export const removeIde = (id) => invoke("remove_ide", { id });
export const setDefaultIde = (id) => invoke("set_default_ide", { id });
export const openProject = (projectPath, ideId) =>
  invoke("open_project", { projectPath, ideId: ideId ?? null });
export const scanOpenProjects = (names) => invoke("scan_open_projects", { names });
export const setProjectIde = (projectPath, ideId) =>
  invoke("set_project_ide", { projectPath, ideId: ideId ?? null });
export const setOpenInTabs = (enabled) => invoke("set_open_in_tabs", { enabled });
export const openRepoUrl = (path) => invoke("open_repo_url", { path });
export const fastStart = (projectPath, branch, base) =>
  invoke("fast_start", { projectPath, branch, base: base ?? null });
export const openLastMr = (path) => invoke("open_last_mr", { path });
export const togglePin = (path) => invoke("toggle_pin", { path });
export const setSectionPrefs = (prefs) => invoke("set_section_prefs", prefs);
export const setWindowPrefs = (prefs) => invoke("set_window_prefs", prefs);
export const addBaseBranch = (name) => invoke("add_base_branch", { name });
export const removeBaseBranch = (name) => invoke("remove_base_branch", { name });
export const setDefaultBaseBranch = (name) => invoke("set_default_base_branch", { name });
export const setShortcut = (accel) => invoke("set_shortcut", { accel });
export const quitApp = () => invoke("quit_app");

export const pickDirectory = (options) =>
  openDialog({ directory: true, multiple: false, ...options });

/// Hide the main window (Spotlight-like: opening a project tucks naeasy
/// away). Goes through the backend's single hide path so transient UI is
/// reset while the window is invisible.
export function hideWindow() {
  invoke("hide_window").catch(() => {
    if (getCurrentWindow) {
      try {
        getCurrentWindow().hide();
      } catch {}
    }
  });
}

/// Fires when the set of installed keyboard layouts changes (added/removed/
/// changed) — payload is the rebuilt layout maps for layout-aware search.
export const onLayoutsChanged = (cb) => window.__TAURI__.event.listen("layouts-changed", cb);
/// Fires every time the popover window is shown (hotkey, tray, dock reopen).
export const onShown = (cb) => window.__TAURI__.event.listen("naeasy://shown", cb);
/// Fires right after the window hides — reset transient UI invisibly.
export const onHidden = (cb) => window.__TAURI__.event.listen("naeasy://hidden", cb);

export const autostart = {
  available: !!autostartPlugin,
  enable: () => autostartPlugin.enable(),
  disable: () => autostartPlugin.disable(),
  isEnabled: () => autostartPlugin.isEnabled(),
};
