// Type-to-search + arrow/Enter navigation + Esc handling.
// Works the moment the window is focused — no need to click the search field.

import { $ } from "./dom.js";
import { store } from "./store.js";
import { render, moveSelection, selectedProjectPath, hideIdePicker, hideFastStart } from "./tree.js";
import { closeSettings } from "./settings.js";

export function initKeyboard({ openProject }) {
  $("#search").addEventListener("input", (e) => {
    store.filter = e.target.value.trim().toLowerCase();
    render();
  });

  document.addEventListener("keydown", (e) => {
    if (!$("#settings").classList.contains("hidden")) return; // settings open
    if (!$("#fast-start").classList.contains("hidden")) return; // fast-start open
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
      const path = selectedProjectPath();
      if (path) openProject(path, null);
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
    store.filter = search.value.trim().toLowerCase();
    render();
  });

  // Esc hides the window-like panels / popovers.
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      hideIdePicker();
      hideFastStart();
      closeSettings();
    }
  });
}
