/* Display helpers shared by the pages. The handshake itself is the node's
   /assets/ember.js (Hearthloom >= 0.5.74), loaded before this file. */
const hm = (s) => `${Math.floor(s / 3600)}h ${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}m`;

// Escape stored text (app names, dates, hostnames) before it goes into innerHTML.
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) =>
  ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
