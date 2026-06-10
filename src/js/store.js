// App state + persistence (localStorage). Mutable singleton — modules read
// and write `store.*`; rendering is triggered explicitly by callers.

export const store = {
  state: { workspaces: [], ides: [], default_ide_id: null, recents: {}, shortcut: "" },
  filter: "",
  openNow: {}, // project folder name -> IDE process name (currently open)
  selectedPath: null, // keyboard-selected project row
};

// Collapsed-node memory (persisted across launches).
const COLLAPSE_KEY = "naeasy.collapsed";
export const collapsed = new Set(JSON.parse(localStorage.getItem(COLLAPSE_KEY) || "[]"));
export function saveCollapsed() {
  localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...collapsed]));
}

export function ideName(id) {
  const ide = store.state.ides.find((i) => i.id === id);
  return ide ? ide.name : null;
}

export function gatherRepos() {
  const out = [];
  const walk = (n) => {
    if (n.is_repo) out.push(n.name);
    (n.children || []).forEach(walk);
  };
  store.state.workspaces.forEach((ws) => (ws.tree.children || []).forEach(walk));
  return out;
}

// Map of project path -> name for every repo in the current tree.
export function allRepoMap() {
  const map = {};
  const walk = (n) => {
    if (n.is_repo) map[n.path] = n.name;
    (n.children || []).forEach(walk);
  };
  store.state.workspaces.forEach((ws) => (ws.tree.children || []).forEach(walk));
  return map;
}
