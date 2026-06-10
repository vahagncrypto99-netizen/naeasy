// Shared DOM utilities.

export const $ = (sel) => document.querySelector(sel);

export function setStatus(msg) {
  $("#status").textContent = msg || "";
  if (msg) setTimeout(() => ($("#status").textContent = ""), 2500);
}

export function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c]));
}

export function escapeAttr(s) {
  return escapeHtml(s);
}
