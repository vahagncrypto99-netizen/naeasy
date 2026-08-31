// Shared DOM utilities.

export const $ = (sel) => document.querySelector(sel);

// The bottom bar carries the key hints; a status message takes the row over
// for a couple of seconds, then the hints come back.
export function setStatus(msg) {
  $("#status").textContent = msg || "";
  $("#footer").classList.toggle("has-status", !!msg);
  if (msg) {
    setTimeout(() => {
      $("#status").textContent = "";
      $("#footer").classList.remove("has-status");
    }, 2500);
  }
}

export function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

export function escapeAttr(s) {
  return escapeHtml(s);
}
