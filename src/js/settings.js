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

// ------------------------- IDE list -------------------------

export function renderIdeList() {
  const list = $("#ide-list");
  const select = $("#default-ide-select");

  // Default-IDE dropdown.
  if (!store.state.ides.length) {
    select.innerHTML = `<option value="">No IDE</option>`;
    select.disabled = true;
  } else {
    select.disabled = false;
    select.innerHTML = store.state.ides
      .map(
        (ide) =>
          `<option value="${ide.id}" ${ide.id === store.state.default_ide_id ? "selected" : ""}>${escapeHtml(ide.name)}</option>`
      )
      .join("");
  }

  if (!store.state.ides.length) {
    list.innerHTML = `<p class="hint">No IDEs configured. Use Auto-detect or Add IDE.</p>`;
    return;
  }
  list.innerHTML = store.state.ides
    .map((ide) => {
      const isDef = ide.id === store.state.default_ide_id;
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

export function initSettings({ refresh, applyState }) {
  $("#open-settings").addEventListener("click", () => $("#settings").classList.remove("hidden"));
  $("#settings-back").addEventListener("click", () => $("#settings").classList.add("hidden"));
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

  $("#ide-list").addEventListener("click", (e) => {
    const remove = e.target.closest("[data-remove-ide]");
    if (remove) {
      refresh(() => api.removeIde(remove.getAttribute("data-remove-ide")));
    }
  });

  $("#default-ide-select").addEventListener("change", (e) => {
    const id = e.target.value;
    if (id) refresh(() => api.setDefaultIde(id));
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
