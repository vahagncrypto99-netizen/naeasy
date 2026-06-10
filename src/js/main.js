// naeasy frontend bootstrap — wires modules together and boots the app.

import * as api from "./api.js";
import { $, setStatus } from "./dom.js";
import { store, ideName, gatherRepos } from "./store.js";
import { render, initTree } from "./tree.js";
import { renderIdeList, initSettings, initAutostart, prettyAccel } from "./settings.js";
import { initKeyboard } from "./shortcuts.js";

function applyState(data) {
  store.state = data;
  if (!store.state.recents) store.state.recents = {};
  if (!store.state.project_ides) store.state.project_ides = {};
  const def = ideName(store.state.default_ide_id);
  $("#default-ide-label").textContent = def ? `Default: ${def}` : "No IDE";
  $("#shortcut-btn").textContent = prettyAccel(store.state.shortcut);
  $("#tabs-toggle").checked = !!store.state.open_in_tabs;
  render();
  renderIdeList();
}

async function refresh(fn) {
  try {
    const data = await fn();
    applyState(data);
  } catch (e) {
    setStatus(String(e));
  }
}

// ------------------------- Actions -------------------------

async function addWorkspace() {
  const selected = await api.pickDirectory({ title: "Select a workspace folder" });
  if (!selected) return;
  setStatus("Scanning…");
  await refresh(() => api.addWorkspace(selected));
  setStatus("Workspace added");
}

async function openProject(path, ideId) {
  try {
    await api.openProject(path, ideId);
    setStatus("Opening…");
    // Launcher behavior: clear the filter and tuck the window away.
    const search = $("#search");
    search.value = "";
    store.filter = "";
    render();
    api.hideWindow();
  } catch (e) {
    setStatus(String(e));
  }
}

// ------------------------- Open-now detection -------------------------

async function refreshOpenNow() {
  try {
    const names = [...new Set(gatherRepos())];
    if (!names.length) {
      store.openNow = {};
      return;
    }
    const map = (await api.scanOpenProjects(names)) || {};
    if (JSON.stringify(map) !== JSON.stringify(store.openNow)) {
      store.openNow = map;
      render();
    }
  } catch (e) {
    // Accessibility not granted or no IDE running — skip silently.
  }
}

// ------------------------- Wiring + boot -------------------------

initTree({
  openProject,
  addWorkspace,
  removeWorkspace: (id) => refresh(() => api.removeWorkspace(id)),
  // Picking an IDE from a project's badge popover is remembered for that
  // project (null = clear the override), then the project opens with it.
  pickProjectIde: async (path, ideId) => {
    await refresh(() => api.setProjectIde(path, ideId));
    openProject(path, ideId);
  },
});
initSettings({ refresh, applyState });
initKeyboard({ openProject });

$("#add-workspace").addEventListener("click", addWorkspace);
$("#rescan").addEventListener("click", () => {
  setStatus("Rescanning…");
  refresh(() => api.rescan());
});

(async function init() {
  try {
    let data = await api.getData();
    // First launch: auto-detect IDEs so the app is useful immediately.
    if (data.ides.length === 0) {
      data = await api.detectIdes();
    }
    applyState(data);
    initAutostart();
    refreshOpenNow();
    setInterval(refreshOpenNow, 6000);
    window.addEventListener("focus", refreshOpenNow);
    // Auto-rescan the workspace tree every 5 minutes — a cheap directory walk
    // (skips node_modules etc.), so new/removed repos appear by themselves.
    setInterval(() => refresh(() => api.rescan()), 5 * 60 * 1000);
  } catch (e) {
    setStatus(String(e));
  }
})();
