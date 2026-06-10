// Tree rendering, keyboard selection and the per-project IDE picker.
// Actions (open project, add/remove workspace) are injected via initTree()
// so this module never calls the backend directly.

import { $, escapeHtml, escapeAttr, setStatus } from "./dom.js";
import { store, collapsed, saveCollapsed, ideName } from "./store.js";
import { recentEntries } from "./recents.js";

// Inline SVG icons rendered inside rounded "chips".
const ICON_FOLDER = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M3 7.5A1.5 1.5 0 0 1 4.5 6h3.8l2 2H19.5A1.5 1.5 0 0 1 21 9.5v7A1.5 1.5 0 0 1 19.5 18h-15A1.5 1.5 0 0 1 3 16.5z"/></svg>`;
const ICON_REPO = `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="6" y1="4" x2="6" y2="15"/><circle cx="18" cy="7" r="2.6"/><circle cx="6" cy="18" r="2.6"/><path d="M18 9.6c0 4-3.5 4.4-9 5.2"/></svg>`;

let handlers = {
  openProject: () => {},
  addWorkspace: () => {},
  removeWorkspace: () => {},
  pickProjectIde: () => {},
};

const content = () => $("#content");

export function prettyIde(p) {
  return p === "Code" ? "VS Code" : p;
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
  const onIde = store.openNow[node.name];
  if (onIde) {
    const nm = prettyIde(onIde);
    return `<span class="meta open-now" title="Open now in ${escapeAttr(nm)}">● ${escapeHtml(nm)}</span>`;
  }
  const rec = store.state.recents[node.path];
  if (rec && rec.last_opened) {
    return `<span class="meta last-seen" title="Last opened in ${escapeAttr(rec.ide)}">${escapeHtml(relTime(rec.last_opened))}</span>`;
  }
  return "";
}

// ------------------------- Tree rendering -------------------------

function matchesFilter(node) {
  if (!store.filter) return true;
  if (node.name.toLowerCase().includes(store.filter)) return true;
  return node.children.some(matchesFilter);
}

function renderNode(node, depth) {
  if (!matchesFilter(node)) return "";
  const hasChildren = node.children.length > 0;
  const isCollapsed = collapsed.has(node.path) && !store.filter;
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
    // The project's remembered IDE wins over the default one.
    const own = ideName((store.state.project_ides || {})[node.path]);
    const label = own || ideName(store.state.default_ide_id) || "set IDE";
    html += `<span class="badge ${own ? "badge-own" : ""}" data-badge="${escapeAttr(node.path)}" title="IDE for this project — click to change">${escapeHtml(label)}</span>`;
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

export function render() {
  const el = content();
  const prevScroll = el.scrollTop;
  if (!store.state.workspaces.length) {
    el.innerHTML = `<div class="empty">No workspaces yet.<br/>Add a folder and naeasy will find every Git project inside.<br/><button class="ghost-btn" id="empty-add">Add workspace…</button></div>`;
    $("#empty-add")?.addEventListener("click", handlers.addWorkspace);
    return;
  }

  let html = "";

  // Recent section (newest first) — shown only when not filtering.
  if (!store.filter) {
    const recents = recentEntries();
    if (recents.length) {
      const rc = collapsed.has("recent");
      html += `<div class="ws recents ${rc ? "collapsed" : ""}">`;
      html += `<div class="ws-head" data-recent>`;
      html += `<span class="twisty">${rc ? "▸" : "▾"}</span>`;
      html += `<span class="ws-name">Recent</span>`;
      html += `<span class="count">${recents.length}</span>`;
      html += `</div><div class="children">`;
      for (const r of recents) {
        html += renderNode({ name: r.name, path: r.path, is_repo: true, children: [] }, 0);
      }
      html += `</div></div>`;
    }
  }

  for (const ws of store.state.workspaces) {
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
  el.innerHTML = html;
  el.scrollTop = prevScroll;
  updateSelection();
}

// ------------------------- Keyboard selection -------------------------

function visibleRepoRows() {
  // Exclude the Recent section so arrow navigation only walks the main tree.
  return [...content().querySelectorAll(".row.repo")].filter(
    (r) => r.offsetParent !== null && !r.closest(".recents")
  );
}

function applySelection(rows) {
  rows.forEach((r) => r.classList.toggle("selected", r.dataset.path === store.selectedPath));
}

export function updateSelection() {
  const rows = visibleRepoRows();
  if (!rows.length) {
    store.selectedPath = null;
    return;
  }
  if (!rows.some((r) => r.dataset.path === store.selectedPath)) {
    store.selectedPath = rows[0].dataset.path;
  }
  applySelection(rows);
}

export function moveSelection(delta) {
  const rows = visibleRepoRows();
  if (!rows.length) return;
  let idx = rows.findIndex((r) => r.dataset.path === store.selectedPath);
  idx = idx === -1 ? 0 : Math.max(0, Math.min(rows.length - 1, idx + delta));
  store.selectedPath = rows[idx].dataset.path;
  applySelection(rows);
  rows[idx].scrollIntoView({ block: "nearest" });
}

function toggleCollapse(key) {
  if (collapsed.has(key)) collapsed.delete(key);
  else collapsed.add(key);
  saveCollapsed();
  render();
}

// ------------------------- IDE picker popover -------------------------

function showIdePicker(anchorEl, projectPath) {
  const picker = $("#ide-picker");
  if (!store.state.ides.length) {
    setStatus("Add an IDE first (⚙)");
    return;
  }
  // Picking here is remembered for this project. The dot marks the current
  // choice; "Default" clears the override and follows the global default.
  const own = (store.state.project_ides || {})[projectPath] || "";
  const defName = ideName(store.state.default_ide_id) || "—";
  picker.innerHTML =
    `<div class="pick" data-pick-ide="" data-pick-path="${escapeAttr(projectPath)}">
      <span class="dot">${own ? "" : "●"}</span><span>Default (${escapeHtml(defName)})</span>
    </div>` +
    store.state.ides
      .map((ide) => {
        const isCur = ide.id === own;
        return `<div class="pick" data-pick-ide="${ide.id}" data-pick-path="${escapeAttr(projectPath)}">
        <span class="dot">${isCur ? "●" : ""}</span><span>${escapeHtml(ide.name)}</span>
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

export function hideIdePicker() {
  $("#ide-picker").classList.add("hidden");
}

// ------------------------- Event wiring -------------------------

export function initTree(h) {
  handlers = { ...handlers, ...h };

  content().addEventListener("click", (e) => {
    const badge = e.target.closest("[data-badge]");
    if (badge) {
      e.stopPropagation();
      showIdePicker(badge, badge.getAttribute("data-badge"));
      return;
    }

    const removeWs = e.target.closest("[data-remove-ws]");
    if (removeWs) {
      e.stopPropagation();
      handlers.removeWorkspace(removeWs.getAttribute("data-remove-ws"));
      return;
    }

    const wsHead = e.target.closest("[data-ws]");
    if (wsHead) {
      const key = "ws:" + wsHead.getAttribute("data-ws");
      toggleCollapse(key);
      return;
    }

    const recentHead = e.target.closest("[data-recent]");
    if (recentHead) {
      toggleCollapse("recent");
      return;
    }

    const row = e.target.closest(".row");
    if (!row) return;
    const path = row.getAttribute("data-path");
    const isRepo = row.getAttribute("data-repo") === "true";
    if (isRepo) {
      handlers.openProject(path, null);
    } else {
      toggleCollapse(path);
    }
  });

  $("#ide-picker").addEventListener("click", (e) => {
    const pick = e.target.closest("[data-pick-ide]");
    if (!pick) return;
    // Remember the choice for this project (empty id = back to default), then open.
    handlers.pickProjectIde(
      pick.getAttribute("data-pick-path"),
      pick.getAttribute("data-pick-ide") || null
    );
    hideIdePicker();
  });

  document.addEventListener("click", (e) => {
    if (!e.target.closest("#ide-picker") && !e.target.closest("[data-badge]")) {
      hideIdePicker();
    }
  });
}
