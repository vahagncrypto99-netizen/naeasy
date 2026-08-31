// naeasy frontend bootstrap — wires modules together and boots the app.

import * as api from "./api.js";
import { $, setStatus } from "./dom.js";
import { store, ideName, gatherRepos } from "./store.js";
import { render, initTree } from "./tree.js";
import {
  renderIdeList,
  renderBranches,
  renderPrefs,
  initSettings,
  initAutostart,
  closeSettings,
  prettyAccel,
} from "./settings.js";
import { hideIdePicker, hideFastStart } from "./tree.js";
import { initKeyboard } from "./shortcuts.js";

// macOS gets translucent surfaces over the native vibrancy material.
if (navigator.userAgent.includes("Mac")) {
  document.body.classList.add("mac");
}

function applyState(data) {
  store.state = data;
  if (!store.state.recents) store.state.recents = {};
  if (!store.state.project_ides) store.state.project_ides = {};
  $("#shortcut-btn").textContent = prettyAccel(store.state.shortcut);
  $("#tabs-toggle").checked = !!store.state.open_in_tabs;
  render();
  renderIdeList();
  renderBranches();
  renderPrefs();
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

// ------------------------- Keyboard layouts -------------------------

// Pull the OS keyboard-layout maps into the store so search can convert a
// query typed under the wrong layout. Re-render if a filter is active so
// results update immediately. Best-effort: empty on unsupported platforms.
async function refreshLayoutMaps() {
  try {
    store.layoutMaps = (await api.getLayoutMaps()) || [];
    if (store.filter) render();
  } catch {
    store.layoutMaps = [];
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
  openRepoUrl: async (path) => {
    try {
      await api.openRepoUrl(path);
    } catch (e) {
      setStatus(String(e));
    }
  },
  togglePin: (path) => refresh(() => api.togglePin(path)),
  openLastMr: async (path) => {
    try {
      await api.openLastMr(path);
    } catch (e) {
      setStatus(String(e));
    }
  },
  fastStart: async (path, branch, base) => {
    setStatus(`Starting ${branch}…`);
    try {
      await api.fastStart(path, branch, base);
      // Same launcher behavior as opening a project.
      const search = $("#search");
      search.value = "";
      store.filter = "";
      await refresh(() => api.getData()); // pick up remembered base + recents
      api.hideWindow();
      setStatus(`On ${branch}`);
    } catch (e) {
      setStatus(String(e));
    }
  },
});
initSettings({ refresh, applyState });
initKeyboard({ openProject });

// Transient UI (settings panel, popovers) is reset the moment the window
// HIDES — while invisible, so the next show never flashes a stale panel.
// The shown-reset stays as a safety net (no-op when already clean).
const resetTransientUi = () => {
  closeSettings();
  hideIdePicker();
  hideFastStart();
};
api.onHidden(resetTransientUi);
api.onShown(resetTransientUi);

// A launcher types the moment it appears: every show puts the caret in the
// filter box, so no click on the field is needed. DOM focus is set even
// before the window manager hands the keyboard over — the keystrokes land in
// the field as soon as it does — and the window-focus listener is the backstop
// for a show that never emitted the event (single-instance relaunch, alt-tab).
function focusSearch() {
  if (!$("#settings").classList.contains("hidden")) return; // settings own the caret
  if (!$("#fast-start").classList.contains("hidden")) return; // so does fast-start
  const search = $("#search");
  search.focus();
  search.select();
}
api.onShown(focusSearch);
window.addEventListener("focus", focusSearch);

// Keep layout maps fresh: the backend pushes a new set when layouts change,
// and we re-pull on every window show as a fallback for any missed event.
api.onLayoutsChanged((e) => {
  store.layoutMaps = e.payload || [];
  if (store.filter) render();
});
api.onShown(refreshLayoutMaps);

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
    focusSearch();
    initAutostart();
    refreshLayoutMaps();
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
