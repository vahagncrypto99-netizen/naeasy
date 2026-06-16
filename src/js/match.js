// Query matching: exact substring + keyboard-layout-aware candidates + a
// typo-tolerant fuzzy-substring. Pure and side-effect free so it can be unit
// tested in isolation; `tree.js` calls `matches()` as its filter predicate.
//
// Layout maps are plain `{ char: latinChar }` dictionaries built from the OS
// (see infra/keyboard_layouts on the backend). We try every map in BOTH
// directions; since we match against a known set of project names, a
// wrong-layout conversion just produces a string that matches nothing.

// Min edit distance of `query` against the best-aligned substring of `text`.
// First DP row is zeroed (match may start anywhere) and we take the min over
// the last row (match may end anywhere) — i.e. approximate substring search,
// so a prefix like "grow" matches "growpay" at distance 0.
export function fuzzySubstring(query, text) {
  const m = query.length;
  const n = text.length;
  if (m === 0) return 0;
  if (n === 0) return m;

  let prev = new Array(n + 1).fill(0); // empty query row — free leading
  let curr = new Array(n + 1);
  for (let i = 1; i <= m; i++) {
    curr[0] = i; // empty text prefix — i deletions
    const qi = query.charCodeAt(i - 1);
    for (let j = 1; j <= n; j++) {
      const cost = qi === text.charCodeAt(j - 1) ? 0 : 1;
      const sub = prev[j - 1] + cost;
      const del = prev[j] + 1; // skip a query char
      const ins = curr[j - 1] + 1; // skip a text char inside the window
      curr[j] = sub < del ? (sub < ins ? sub : ins) : del < ins ? del : ins;
    }
    [prev, curr] = [curr, prev];
  }
  // `prev` now holds the last computed row.
  let best = prev[0];
  for (let j = 1; j <= n; j++) if (prev[j] < best) best = prev[j];
  return best;
}

// Allowed edits, scaled to query length: short queries must match exactly
// (otherwise 2–3 letters drag in noise), longer ones tolerate ~1 error / 4.
export function threshold(len) {
  if (len <= 3) return 0;
  if (len <= 7) return 1;
  return 2;
}

// Remap each character through a `{ from: to }` dictionary; unmapped chars
// pass through unchanged.
export function convert(query, map) {
  let out = "";
  for (const ch of query) out += map[ch] ?? ch;
  return out;
}

// Invert a `{ char: latin }` map to `{ latin: char }`. On collisions the last
// entry wins — acceptable for the closed-world filter.
function invert(map) {
  const out = {};
  for (const k in map) out[map[k]] = k;
  return out;
}

// All forms of `query` worth testing: the raw input plus each layout map
// applied in both directions (script→latin and latin→script). Deduped.
export function candidates(query, maps) {
  const set = new Set([query]);
  for (const map of maps || []) {
    const dict = map && map.map ? map.map : map; // accept {id,map} or bare dict
    if (!dict) continue;
    set.add(convert(query, dict));
    set.add(convert(query, invert(dict)));
  }
  return [...set];
}

// True if `name` matches `query`, considering layout conversions and typos.
export function matches(query, name, maps) {
  const q = (query || "").toLowerCase();
  if (!q) return true;
  const text = (name || "").toLowerCase();
  for (const cand of candidates(q, maps)) {
    if (!cand) continue;
    if (text.includes(cand)) return true;
    if (fuzzySubstring(cand, text) <= threshold(cand.length)) return true;
  }
  return false;
}
