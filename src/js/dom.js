// Shared DOM utilities.

export const $ = (sel) => document.querySelector(sel);

// The footer is a transient status toast — visible only while a message is up.
export function setStatus(msg) {
  $("#status").textContent = msg || "";
  $("#footer").classList.toggle("visible", !!msg);
  if (msg) {
    setTimeout(() => {
      $("#status").textContent = "";
      $("#footer").classList.remove("visible");
    }, 2500);
  }
}

export function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

export function escapeAttr(s) {
  return escapeHtml(s);
}
