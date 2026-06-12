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
  const select = $("#base-branch-select");
  const branches = store.state.base_branches || ["master"];
  const def = store.state.default_base_branch || branches[0];
  select.innerHTML = branches
    .map(
      (b) =>
        `<option value="${escapeHtml(b)}" ${b === def ? "selected" : ""}>${escapeHtml(b)}</option>`
    )
    .join("");
  $("#remove-base-branch").disabled = branches.length <= 1;
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

function openSettings() {
  $("#settings").classList.remove("hidden");
  document.body.classList.add("settings-open");
}

export function initSettings({ refresh, applyState }) {
  $("#open-settings").addEventListener("click", openSettings);
  $("#settings-back").addEventListener("click", closeSettings);
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

  // Fast-start base branches.
  $("#base-branch-select").addEventListener("change", (e) => {
    if (e.target.value) refresh(() => api.setDefaultBaseBranch(e.target.value));
  });
  $("#remove-base-branch").addEventListener("click", () => {
    const name = $("#base-branch-select").value;
    if (name) refresh(() => api.removeBaseBranch(name));
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
