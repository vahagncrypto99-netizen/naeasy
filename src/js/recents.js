// "Recent" section data: most recently opened projects, newest first.

import { store, allRepoMap } from "./store.js";

// Top-N most recently opened projects (that still exist), newest first.
export function recentEntries(limit = 8) {
  const repoMap = allRepoMap();
  return Object.entries(store.state.recents || {})
    .filter(([p]) => repoMap[p])
    .sort((a, b) => (b[1].last_opened || 0) - (a[1].last_opened || 0))
    .slice(0, limit)
    .map(([path]) => ({ path, name: repoMap[path] }));
}
