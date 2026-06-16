// Settings panel: IDE list, default-IDE select, shortcut recorder, autostart,
// quit. Backend calls go through api.js; refresh/applyState are injected.

import * as api from "./api.js";
import { $, escapeHtml, setStatus } from "./dom.js";
import { store } from "./store.js";

export function prettyAccel(a) {
  if (!a) return "—";
  return a
    .replace(/CmdOrCtrl|Command|Cmd|Super|Meta/gi, "⌘")
    .replace(/Control|Ctrl/gi, "⌃")
    .replace(/Option|Alt/gi, "⌥")
    .replace(/Shift/gi, "⇧")
    .replace(/\+/g, "");
}

// ------------------------- Default-IDE select -------------------------

export function renderIdeList() {
  const select = $("#default-ide-select");
  const removeBtn = $("#remove-ide");

  if (!store.state.ides.length) {
    select.innerHTML = `<option value="">No IDE</option>`;
    select.disabled = true;
    removeBtn.disabled = true;
    return;
  }
  select.disabled = false;
  removeBtn.disabled = false;
  select.innerHTML = store.state.ides
    .map(
      (ide) =>
        `<option value="${ide.id}" ${ide.id === store.state.default_ide_id ? "selected" : ""}>${escapeHtml(ide.name)}</option>`
    )
    .join("");
}

// ------------------------- Fast-start base branches -------------------------

export function renderBranches() {
  const branches = store.state.base_branches || ["master"];
  const def = store.state.default_base_branch || branches[0];
  const removable = branches.length > 1;
  $("#branch-chips").innerHTML = branches
    .map(
      (b) =>
        `<span class="chip-branch ${b === def ? "active" : ""}" data-branch="${escapeHtml(b)}" title="Make default">${escapeHtml(b)}${
          removable ? `<span class="chip-x" data-del-branch="${escapeHtml(b)}" title="Remove">✕</span>` : ""
        }</span>`
    )
    .join("");
}

// ------------------------- Window & sections prefs -------------------------

export function renderPrefs() {
  const s = store.state;
  $("#float-toggle").checked = !!s.window_float;
  $("#fixed-toggle").checked = !!s.window_fixed;
  $("#fixed-size-row").style.display = s.window_fixed ? "flex" : "none";
  if (s.window_fixed && s.fixed_size) {
    const cur = `${s.fixed_size[0]}x${s.fixed_size[1]}`;
    const preset = $("#size-preset");
    const match = [...preset.options].find((o) => o.value === cur);
    if (match) {
      preset.value = cur;
    } else {
      // Custom size after +/- tweaks — show it as a transient option.
      let custom = preset.querySelector("option[data-custom]");
      if (!custom) {
        custom = document.createElement("option");
        custom.setAttribute("data-custom", "");
        preset.appendChild(custom);
      }
      custom.value = cur;
      custom.textContent = `Custom · ${s.fixed_size[0]}×${s.fixed_size[1]}`;
      preset.value = cur;
    }
  }
  $("#recent-toggle").checked = !!s.show_recent;
  $("#max-recent").value = s.max_recent ?? 3;
  $("#pinned-toggle").checked = !!s.show_pinned;
  $("#max-pinned").value = s.max_pinned ?? 3;

  // Dragging only makes sense for a floating window.
  document.querySelectorAll(".header, .header-left, .title").forEach((el) => {
    if (s.window_float) el.setAttribute("data-tauri-drag-region", "");
    else el.removeAttribute("data-tauri-drag-region");
  });
}

// ------------------------- Shortcut recorder -------------------------

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

// ------------------------- Autostart -------------------------

// Launch at login (autostart). Auto-enabled once on first run, then user-controlled.
export async function initAutostart() {
  if (!api.autostart.available) return;
  try {
    if (!localStorage.getItem("naeasy.autostart_init")) {
      await api.autostart.enable();
      localStorage.setItem("naeasy.autostart_init", "1");
    }
    $("#autostart-toggle").checked = await api.autostart.isEnabled();
  } catch (e) {
    setStatus(String(e));
  }
}

// ------------------------- Event wiring -------------------------

/// The tree must not shine through the translucent settings panel — hide it
/// while settings are open (the glass then shows the desktop, not our DOM).
export function closeSettings() {
  $("#settings").classList.add("hidden");
  document.body.classList.remove("settings-open");
}

// Tab bar: show one .tab-panel at a time, highlight its tab.
function selectTab(name) {
  document.querySelectorAll("#settings-tabs .tab").forEach((t) =>
    t.classList.toggle("active", t.dataset.tab === name)
  );
  document.querySelectorAll(".tab-panel").forEach((p) => {
    p.hidden = p.dataset.tab !== name;
  });
}

let versionLoaded = false;

function openSettings() {
  $("#settings").classList.remove("hidden");
  document.body.classList.add("settings-open");
  selectTab("general"); // always reopen on the first tab
  if (!versionLoaded) {
    versionLoaded = true;
    api
      .getVersion()
      .then((v) => {
        if (v) $("#about-version").textContent = `v${v}`;
      })
      .catch(() => {});
  }
}

export function initSettings({ refresh, applyState }) {
  $("#open-settings").addEventListener("click", openSettings);
  $("#settings-back").addEventListener("click", closeSettings);

  $("#settings-tabs").addEventListener("click", (e) => {
    const tab = e.target.closest(".tab");
    if (tab) selectTab(tab.dataset.tab);
  });
  $("#detect-ides").addEventListener("click", () => refresh(() => api.detectIdes()));
  $("#quit-app").addEventListener("click", () => api.quitApp());

  $("#add-ide").addEventListener("click", async () => {
    const selected = await api.pickDirectory({
      // macOS .app bundles are directories
      defaultPath: "/Applications",
      title: "Select an IDE application",
    });
    if (!selected) return;
    await refresh(() => api.addIde(selected));
    setStatus("IDE added");
  });

  // ✕ removes the IDE currently selected in the Default dropdown.
  $("#remove-ide").addEventListener("click", () => {
    const id = $("#default-ide-select").value;
    if (id) refresh(() => api.removeIde(id));
  });

  $("#default-ide-select").addEventListener("change", (e) => {
    const id = e.target.value;
    if (id) refresh(() => api.setDefaultIde(id));
  });

  // Fast-start base branches: click a chip = make default, ✕ = remove.
  $("#branch-chips").addEventListener("click", (e) => {
    const del = e.target.closest("[data-del-branch]");
    if (del) {
      e.stopPropagation();
      refresh(() => api.removeBaseBranch(del.getAttribute("data-del-branch")));
      return;
    }
    const chip = e.target.closest("[data-branch]");
    if (chip) {
      refresh(() => api.setDefaultBaseBranch(chip.getAttribute("data-branch")));
    }
  });
  const addBranch = async () => {
    const input = $("#new-base-branch");
    const name = input.value.trim();
    if (!name) return;
    await refresh(() => api.addBaseBranch(name));
    input.value = "";
  };
  $("#add-base-branch").addEventListener("click", addBranch);
  $("#new-base-branch").addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      addBranch();
    }
    e.stopPropagation();
  });

  // Window mode + size.
  $("#float-toggle").addEventListener("change", (e) =>
    refresh(() => api.setWindowPrefs({ float: e.target.checked }))
  );
  $("#fixed-toggle").addEventListener("change", (e) =>
    refresh(() => api.setWindowPrefs({ fixed: e.target.checked }))
  );
  $("#size-preset").addEventListener("change", (e) => {
    const [w, h] = e.target.value.split("x").map(Number);
    if (w && h) refresh(() => api.setWindowPrefs({ fixedSize: [w, h] }));
  });
  const nudgeSize = (d) => {
    const s = store.state.fixed_size || [380, 560];
    refresh(() => api.setWindowPrefs({ fixedSize: [s[0] + d, s[1] + d] }));
  };
  $("#size-minus").addEventListener("click", () => nudgeSize(-20));
  $("#size-plus").addEventListener("click", () => nudgeSize(20));

  // Sections.
  $("#recent-toggle").addEventListener("change", (e) =>
    refresh(() => api.setSectionPrefs({ showRecent: e.target.checked }))
  );
  $("#pinned-toggle").addEventListener("change", (e) =>
    refresh(() => api.setSectionPrefs({ showPinned: e.target.checked }))
  );
  $("#max-recent").addEventListener("change", (e) => {
    const n = parseInt(e.target.value, 10);
    if (n >= 1) refresh(() => api.setSectionPrefs({ maxRecent: n }));
  });
  $("#max-pinned").addEventListener("change", (e) => {
    const n = parseInt(e.target.value, 10);
    if (n >= 1) refresh(() => api.setSectionPrefs({ maxPinned: n }));
  });

  // Global shortcut recorder.
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
        btn.textContent = prettyAccel(store.state.shortcut);
        setStatus("Use a modifier + one key");
        return;
      }
      try {
        const data = await api.setShortcut(accel);
        applyState(data);
        setStatus("Shortcut updated");
      } catch (err) {
        btn.textContent = prettyAccel(store.state.shortcut);
        setStatus(String(err));
      }
    };
    document.addEventListener("keydown", onKey, true);
  });

  // Tabs-vs-windows mode for opening projects.
  $("#tabs-toggle").addEventListener("change", (e) => {
    refresh(() => api.setOpenInTabs(e.target.checked));
  });

  const autostartToggle = $("#autostart-toggle");
  autostartToggle.addEventListener("change", async (e) => {
    try {
      if (e.target.checked) await api.autostart.enable();
      else await api.autostart.disable();
      setStatus(e.target.checked ? "Launch at login on" : "Launch at login off");
    } catch (err) {
      setStatus(String(err));
      e.target.checked = await api.autostart.isEnabled().catch(() => false);
    }
  });
}
