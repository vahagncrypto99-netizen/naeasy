// naeasy frontend — uses the global Tauri API (withGlobalTauri = true).
const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const autostart = window.__TAURI__.autostart;
const getCurrentWindow = window.__TAURI__.window?.getCurrentWindow;

let state = { workspaces: [], ides: [], default_ide_id: null, recents: {}, shortcut: "" };
let filter = "";
let openNow = {}; // project folder name -> IDE process name (currently open)
let selectedPath = null; // keyboard-selected project row

// Inline SVG icons rendered inside rounded "chips".
const ICON_FOLDER = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h3.8l2 2H19.5A1.5 1.5 0 0 1 21 9.5v7A1.5 1.5 0 0 1 19.5 18h-15A1.5 1.5 0 0 1 3 16.5z"/></svg>`;
const ICON_REPO = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="4" x2="6" y2="15"/><circle cx="18" cy="7" r="2.6"/><circle cx="6" cy="18" r="2.6"/><path d="M18 9.6c0 4-3.5 4.4-9 5.2"/></svg>`;

// Collapsed-node memory (persisted across launches).
const COLLAPSE_KEY = "naeasy.collapsed";
const collapsed = new Set(JSON.parse(localStorage.getItem(COLLAPSE_KEY) || "[]"));
function saveCollapsed() {
  localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...collapsed]));
}

const $ = (sel) => document.querySelector(sel);
const content = $("#content");

function setStatus(msg) {
  $("#status").textContent = msg || "";
  if (msg) setTimeout(() => ($("#status").textContent = ""), 2500);
}

function ideName(id) {
  const ide = state.ides.find((i) => i.id === id);
  return ide ? ide.name : null;
}

function applyState(data) {
  state = data;
  if (!state.recents) state.recents = {};
  const def = ideName(state.default_ide_id);
  $("#default-ide-label").textContent = def ? `Default: ${def}` : "No IDE";
  $("#shortcut-btn").textContent = prettyAccel(state.shortcut);
  render();
  renderIdeList();
}

function prettyIde(p) {
  return p === "Code" ? "VS Code" : p;
}

function prettyAccel(a) {
  if (!a) return "—";
  return a
    .replace(/CmdOrCtrl|Command|Cmd|Super|Meta/gi, "⌘")
    .replace(/Control|Ctrl/gi, "⌃")
    .replace(/Option|Alt/gi, "⌥")
    .replace(/Shift/gi, "⇧")
    .replace(/\+/g, "");
}

function relTime(sec) {
  const d = Math.floor(Date.now() / 1000) - sec;
  if (d < 60) return "just now";
  if (d < 3600) return Math.floor(d / 60) + "m ago";
  if (d < 86400) return Math.floor(d / 3600) + "h ago";
  if (d < 604800) return Math.floor(d / 86400) + "d ago";
  return Math.floor(d / 604800) + "w ago";
}

function metaHtml(node) {
  const onIde = openNow[node.name];
  if (onIde) {
    const nm = prettyIde(onIde);
    return `<span class="meta open-now" title="Open now in ${escapeAttr(nm)}">● ${escapeHtml(nm)}</span>`;
  }
  const rec = state.recents[node.path];
  if (rec && rec.last_opened) {
    return `<span class="meta last-seen" title="Last opened in ${escapeAttr(rec.ide)}">${escapeHtml(relTime(rec.last_opened))}</span>`;
  }
  return "";
}

// ------------------------- Tree rendering -------------------------

function matchesFilter(node) {
  if (!filter) return true;
  if (node.name.toLowerCase().includes(filter)) return true;
  return node.children.some(matchesFilter);
}

function renderNode(node, depth) {
  if (!matchesFilter(node)) return "";
  const hasChildren = node.children.length > 0;
  const isCollapsed = collapsed.has(node.path) && !filter;
  const cls = node.is_repo ? "repo" : "group";
  const twisty = hasChildren ? (isCollapsed ? "▸" : "▾") : "";
  const icon = node.is_repo ? ICON_REPO : ICON_FOLDER;

  let html = `<div class="node ${isCollapsed ? "collapsed" : ""}">`;
  html += `<div class="row ${cls}" data-path="${escapeAttr(node.path)}" data-repo="${node.is_repo}" data-name="${escapeAttr(node.name)}">`;
  html += `<span class="twisty">${twisty}</span>`;
  html += `<span class="chip ${cls}-chip">${icon}</span>`;
  html += `<span class="label">${escapeHtml(node.name)}</span>`;
  if (node.is_repo) {
    html += metaHtml(node);
    const def = ideName(state.default_ide_id) || "set IDE";
    html += `<span class="badge" data-badge="${escapeAttr(node.path)}">${escapeHtml(def)}</span>`;
  }
  html += `</div>`;
  if (hasChildren) {
    html += `<div class="children">`;
    for (const child of node.children) html += renderNode(child, depth + 1);
    html += `</div>`;
  }
  html += `</div>`;
  return html;
}

function render() {
  const prevScroll = content.scrollTop;
  if (!state.workspaces.length) {
    content.innerHTML = `<div class="empty">No workspaces yet.<br/>Add a folder and naeasy will find every Git project inside.<br/><button class="ghost-btn" id="empty-add">Add workspace…</button></div>`;
    $("#empty-add")?.addEventListener("click", addWorkspace);
    return;
  }

  let html = "";
  for (const ws of state.workspaces) {
    const wsCollapsed = collapsed.has("ws:" + ws.id);
    html += `<div class="ws ${wsCollapsed ? "collapsed" : ""}">`;
    html += `<div class="ws-head" data-ws="${ws.id}">`;
    html += `<span class="twisty">${wsCollapsed ? "▸" : "▾"}</span>`;
    html += `<span class="ws-name">${escapeHtml(ws.name)}</span>`;
    html += `<span class="count">${ws.repo_count}</span>`;
    html += `<span class="ws-remove" data-remove-ws="${ws.id}" title="Remove workspace">✕</span>`;
    html += `</div>`;
    html += `<div class="children">`;
    for (const child of ws.tree.children) html += renderNode(child, 0);
    html += `</div></div>`;
  }
  content.innerHTML = html;
  content.scrollTop = prevScroll;
  updateSelection();
}

// ------------------------- Keyboard selection -------------------------

function visibleRepoRows() {
  return [...content.querySelectorAll(".row.repo")].filter(
    (r) => r.offsetParent !== null
  );
}

function applySelection(rows) {
  rows.forEach((r) => r.classList.toggle("selected", r.dataset.path === selectedPath));
}

function updateSelection() {
  const rows = visibleRepoRows();
  if (!rows.length) {
    selectedPath = null;
    return;
  }
  if (!rows.some((r) => r.dataset.path === selectedPath)) {
    selectedPath = rows[0].dataset.path;
  }
  applySelection(rows);
}

function moveSelection(delta) {
  const rows = visibleRepoRows();
  if (!rows.length) return;
  let idx = rows.findIndex((r) => r.dataset.path === selectedPath);
  idx = idx === -1 ? 0 : Math.max(0, Math.min(rows.length - 1, idx + delta));
  selectedPath = rows[idx].dataset.path;
  applySelection(rows);
  rows[idx].scrollIntoView({ block: "nearest" });
}

// ------------------------- Open-now detection -------------------------

function gatherRepos() {
  const out = [];
  const walk = (n) => {
    if (n.is_repo) out.push(n.name);
    (n.children || []).forEach(walk);
  };
  state.workspaces.forEach((ws) => (ws.tree.children || []).forEach(walk));
  return out;
}

async function refreshOpenNow() {
  try {
    const names = [...new Set(gatherRepos())];
    if (!names.length) {
      openNow = {};
      return;
    }
    const map = (await invoke("scan_open_projects", { names })) || {};
    if (JSON.stringify(map) !== JSON.stringify(openNow)) {
      openNow = map;
      render();
    }
  } catch (e) {
    // Accessibility not granted or no IDE running — skip silently.
  }
}

// ------------------------- IDE list (settings) -------------------------

function renderIdeList() {
  const list = $("#ide-list");
  const select = $("#default-ide-select");

  // Default-IDE dropdown.
  if (!state.ides.length) {
    select.innerHTML = `<option value="">No IDE</option>`;
    select.disabled = true;
  } else {
    select.disabled = false;
    select.innerHTML = state.ides
      .map(
        (ide) =>
          `<option value="${ide.id}" ${ide.id === state.default_ide_id ? "selected" : ""}>${escapeHtml(ide.name)}</option>`
      )
      .join("");
  }

  if (!state.ides.length) {
    list.innerHTML = `<p class="hint">No IDEs configured. Use Auto-detect or Add IDE.</p>`;
    return;
  }
  list.innerHTML = state.ides
    .map((ide) => {
      const isDef = ide.id === state.default_ide_id;
      return `<li class="ide-item">
        <div style="flex:1; min-width:0">
          <div class="ide-name">${escapeHtml(ide.name)}${isDef ? ' <span class="ide-default-tag">default</span>' : ""}</div>
          <div class="ide-path">${escapeHtml(ide.path)}</div>
        </div>
        <span class="ide-remove" data-remove-ide="${ide.id}" title="Remove">✕</span>
      </li>`;
    })
    .join("");
}

// ------------------------- Actions -------------------------

async function refresh(fn) {
  try {
    const data = await fn();
    applyState(data);
  } catch (e) {
    setStatus(String(e));
  }
}

async function addWorkspace() {
  const selected = await open({ directory: true, multiple: false, title: "Select a workspace folder" });
  if (!selected) return;
  setStatus("Scanning…");
  await refresh(() => invoke("add_workspace", { path: selected }));
  setStatus("Workspace added");
}

async function addIde() {
  const selected = await open({
    directory: true, // macOS .app bundles are directories
    multiple: false,
    defaultPath: "/Applications",
    title: "Select an IDE application",
  });
  if (!selected) return;
  await refresh(() => invoke("add_ide", { path: selected }));
  setStatus("IDE added");
}

async function openProject(path, ideId) {
  try {
    await invoke("open_project", { projectPath: path, ideId: ideId ?? null });
    setStatus("Opening…");
    // Launcher behavior: clear the filter and tuck the window away.
    const search = $("#search");
    search.value = "";
    filter = "";
    render();
    if (getCurrentWindow) {
      try {
        getCurrentWindow().hide();
      } catch {}
    }
  } catch (e) {
    setStatus(String(e));
  }
}

// ------------------------- IDE picker popover -------------------------

function showIdePicker(anchorEl, projectPath) {
  const picker = $("#ide-picker");
  if (!state.ides.length) {
    setStatus("Add an IDE first (⚙)");
    return;
  }
  picker.innerHTML = state.ides
    .map((ide) => {
      const isDef = ide.id === state.default_ide_id;
      return `<div class="pick" data-pick-ide="${ide.id}" data-pick-path="${escapeAttr(projectPath)}">
        <span class="dot">${isDef ? "●" : ""}</span><span>${escapeHtml(ide.name)}</span>
      </div>`;
    })
    .join("");
  const rect = anchorEl.getBoundingClientRect();
  picker.classList.remove("hidden");
  const pw = picker.offsetWidth;
  let left = rect.right - pw;
  if (left < 8) left = 8;
  let top = rect.bottom + 4;
  if (top + picker.offsetHeight > window.innerHeight - 8) {
    top = rect.top - picker.offsetHeight - 4;
  }
  picker.style.left = `${left}px`;
  picker.style.top = `${top}px`;
}

function hideIdePicker() {
  $("#ide-picker").classList.add("hidden");
}

// ------------------------- Event wiring -------------------------

content.addEventListener("click", (e) => {
  const badge = e.target.closest("[data-badge]");
  if (badge) {
    e.stopPropagation();
    showIdePicker(badge, badge.getAttribute("data-badge"));
    return;
  }

  const removeWs = e.target.closest("[data-remove-ws]");
  if (removeWs) {
    e.stopPropagation();
    refresh(() => invoke("remove_workspace", { id: removeWs.getAttribute("data-remove-ws") }));
    return;
  }

  const wsHead = e.target.closest("[data-ws]");
  if (wsHead) {
    const key = "ws:" + wsHead.getAttribute("data-ws");
    toggleCollapse(key);
    return;
  }

  const row = e.target.closest(".row");
  if (!row) return;
  const path = row.getAttribute("data-path");
  const isRepo = row.getAttribute("data-repo") === "true";
  if (isRepo) {
    openProject(path, null);
  } else {
    toggleCollapse(path);
  }
});

function toggleCollapse(key) {
  if (collapsed.has(key)) collapsed.delete(key);
  else collapsed.add(key);
  saveCollapsed();
  render();
}

$("#ide-picker").addEventListener("click", (e) => {
  const pick = e.target.closest("[data-pick-ide]");
  if (!pick) return;
  openProject(pick.getAttribute("data-pick-path"), pick.getAttribute("data-pick-ide"));
  hideIdePicker();
});

document.addEventListener("click", (e) => {
  if (!e.target.closest("#ide-picker") && !e.target.closest("[data-badge]")) {
    hideIdePicker();
  }
});

$("#search").addEventListener("input", (e) => {
  filter = e.target.value.trim().toLowerCase();
  render();
});

$("#add-workspace").addEventListener("click", addWorkspace);
$("#rescan").addEventListener("click", () => {
  setStatus("Rescanning…");
  refresh(() => invoke("rescan"));
});

// Settings panel
$("#open-settings").addEventListener("click", () => $("#settings").classList.remove("hidden"));
$("#settings-back").addEventListener("click", () => $("#settings").classList.add("hidden"));
$("#detect-ides").addEventListener("click", () => refresh(() => invoke("detect_ides")));
$("#add-ide").addEventListener("click", addIde);
$("#quit-app").addEventListener("click", () => invoke("quit_app"));

// Close button — hides the window (no Dock thumbnail). Reopen via the
// menu-bar icon or the global shortcut.
$("#win-close").addEventListener("click", () => {
  try {
    getCurrentWindow && getCurrentWindow().hide();
  } catch (e) {
    setStatus(String(e));
  }
});

// Type-to-search + arrow/Enter navigation. Works the moment the window is
// focused — no need to click the search field first.
document.addEventListener("keydown", (e) => {
  if (!$("#settings").classList.contains("hidden")) return; // settings open
  if (e.metaKey || e.ctrlKey || e.altKey) return;

  // Navigation (works even while the search field has focus).
  if (e.key === "ArrowDown") {
    e.preventDefault();
    moveSelection(1);
    return;
  }
  if (e.key === "ArrowUp") {
    e.preventDefault();
    moveSelection(-1);
    return;
  }
  if (e.key === "Enter") {
    e.preventDefault();
    if (selectedPath) openProject(selectedPath, null);
    return;
  }

  // Type-to-search.
  const ae = document.activeElement;
  if (ae && (ae.tagName === "INPUT" || ae.tagName === "TEXTAREA")) return;
  const search = $("#search");
  if (e.key.length === 1) {
    e.preventDefault();
    search.focus();
    search.value += e.key;
  } else if (e.key === "Backspace") {
    e.preventDefault();
    search.focus();
    search.value = search.value.slice(0, -1);
  } else {
    return;
  }
  filter = search.value.trim().toLowerCase();
  render();
});

// Global shortcut recorder
const SPECIAL_KEYS = {
  ArrowUp: "Up", ArrowDown: "Down", ArrowLeft: "Left", ArrowRight: "Right",
  " ": "Space", Escape: "Esc", Enter: "Enter", Tab: "Tab",
};
function accelFromEvent(e) {
  const mods = [];
  if (e.metaKey) mods.push("CmdOrCtrl");
  if (e.ctrlKey && !e.metaKey) mods.push("Ctrl");
  if (e.altKey) mods.push("Alt");
  if (e.shiftKey) mods.push("Shift");
  if (!mods.length) return null; // a global hotkey needs at least one modifier
  let key = e.key;
  if (key.length === 1) key = key.toUpperCase();
  else key = SPECIAL_KEYS[key] || key;
  return [...mods, key].join("+");
}
let recordingShortcut = false;
$("#shortcut-btn").addEventListener("click", () => {
  if (recordingShortcut) return;
  recordingShortcut = true;
  const btn = $("#shortcut-btn");
  btn.textContent = "Press keys…";
  btn.classList.add("recording");
  const onKey = async (e) => {
    if (["Meta", "Control", "Alt", "Shift"].includes(e.key)) return; // wait for a real key
    e.preventDefault();
    e.stopPropagation();
    document.removeEventListener("keydown", onKey, true);
    recordingShortcut = false;
    btn.classList.remove("recording");
    const accel = accelFromEvent(e);
    if (!accel) {
      btn.textContent = prettyAccel(state.shortcut);
      setStatus("Use a modifier + one key");
      return;
    }
    try {
      const data = await invoke("set_shortcut", { accel });
      applyState(data);
      setStatus("Shortcut updated");
    } catch (err) {
      btn.textContent = prettyAccel(state.shortcut);
      setStatus(String(err));
    }
  };
  document.addEventListener("keydown", onKey, true);
});

$("#ide-list").addEventListener("click", (e) => {
  const remove = e.target.closest("[data-remove-ide]");
  if (remove) {
    refresh(() => invoke("remove_ide", { id: remove.getAttribute("data-remove-ide") }));
  }
});

$("#default-ide-select").addEventListener("change", (e) => {
  const id = e.target.value;
  if (id) refresh(() => invoke("set_default_ide", { id }));
});

// Launch at login (autostart). Auto-enabled once on first run, then user-controlled.
const autostartToggle = $("#autostart-toggle");
async function initAutostart() {
  if (!autostart) return;
  try {
    if (!localStorage.getItem("naeasy.autostart_init")) {
      await autostart.enable();
      localStorage.setItem("naeasy.autostart_init", "1");
    }
    autostartToggle.checked = await autostart.isEnabled();
  } catch (e) {
    setStatus(String(e));
  }
}
autostartToggle.addEventListener("change", async (e) => {
  try {
    if (e.target.checked) await autostart.enable();
    else await autostart.disable();
    setStatus(e.target.checked ? "Launch at login on" : "Launch at login off");
  } catch (err) {
    setStatus(String(err));
    e.target.checked = await autostart.isEnabled().catch(() => false);
  }
});

// Esc hides the window-like panels / popover.
document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    hideIdePicker();
    if (!$("#settings").classList.contains("hidden")) {
      $("#settings").classList.add("hidden");
    }
  }
});

// ------------------------- Helpers -------------------------

function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}
function escapeAttr(s) {
  return escapeHtml(s);
}

// ------------------------- Boot -------------------------

(async function init() {
  try {
    let data = await invoke("get_data");
    // First launch: auto-detect IDEs so the app is useful immediately.
    if (data.ides.length === 0) {
      data = await invoke("detect_ides");
    }
    applyState(data);
    initAutostart();
    refreshOpenNow();
    setInterval(refreshOpenNow, 6000);
    window.addEventListener("focus", refreshOpenNow);
  } catch (e) {
    setStatus(String(e));
  }
})();
