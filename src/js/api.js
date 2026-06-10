// The only module that talks to the Tauri API (withGlobalTauri = true).
// Everything else imports these wrappers.

const { invoke } = window.__TAURI__.core;
const { open: openDialog } = window.__TAURI__.dialog;
const autostartPlugin = window.__TAURI__.autostart;
const getCurrentWindow = window.__TAURI__.window?.getCurrentWindow;

export const getData = () => invoke("get_data");
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
export const setShortcut = (accel) => invoke("set_shortcut", { accel });
export const quitApp = () => invoke("quit_app");

export const pickDirectory = (options) =>
  openDialog({ directory: true, multiple: false, ...options });

/// Hide the main window (Spotlight-like: opening a project tucks naeasy away).
export function hideWindow() {
  if (getCurrentWindow) {
    try {
      getCurrentWindow().hide();
    } catch {}
  }
}

export const autostart = {
  available: !!autostartPlugin,
  enable: () => autostartPlugin.enable(),
  disable: () => autostartPlugin.disable(),
  isEnabled: () => autostartPlugin.isEnabled(),
};
