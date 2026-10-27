/* The ember-side half of the sandbox handshake: Hearthloom's shell
   posts {type:"hearthloom", token, base, palette} into the iframe on
   load. `window.hearth` resolves with a tiny client bound to that
   token — the ONLY authority this page has. Messages count only when
   they come from the window that embeds us (the dashboard shell);
   anything else that can reach this frame is ignored. */
// Two checks: the message comes from the window that embeds us, and that
// window is the dashboard's own origin (the page and the dashboard are
// served by the same node). The source check alone would accept a site
// that frames this page itself.
const fromShell = (ev) =>
  window.parent !== window && ev.source === window.parent && ev.origin === location.origin;

window.hearth = new Promise((resolve) => {
  addEventListener("message", (ev) => {
    if (!fromShell(ev)) return;
    const d = ev.data;
    if (!d || d.type !== "hearthloom") return;
    for (const [k, v] of Object.entries(d.palette || {})) {
      document.documentElement.style.setProperty(k, v);
    }
    const headers = { authorization: `Bearer ${d.token}` };
    resolve({
      base: d.base,
      async kv(key) {
        try {
          const r = await fetch(`${d.base}/v1/plugins/self/kv/${key}`, { headers });
          if (!r.ok) return null;
          return JSON.parse((await r.json()).value);
        } catch { return null; }
      },
      async kvPut(key, value) {
        await fetch(`${d.base}/v1/plugins/self/kv/${key}`, {
          method: "PUT", headers, body: JSON.stringify(value),
        });
      },
      async fileGet(path) {
        try {
          const r = await fetch(`${d.base}/v1/plugins/self/files/${path}`, { headers });
          if (!r.ok) return null;
          return JSON.parse(await r.text());
        } catch { return null; }
      },
      onTimespan(fn) {
        addEventListener("message", (ev) => {
          if (fromShell(ev) && ev.data?.type === "hearthloom-timespan") fn(ev.data.from, ev.data.to);
        });
      },
    });
  });
});
// Auto-report content height so the dashboard sizes our iframe to fit
// (no inner scrollbar — the page flows in the dashboard's own scroll).
{
  const report = () => {
    try {
      parent.postMessage(
        { type: "hearthloom-height", px: Math.ceil(document.documentElement.scrollHeight) },
        "*"
      );
    } catch { /* not framed */ }
  };
  addEventListener("load", report);
  if (window.ResizeObserver) new ResizeObserver(report).observe(document.documentElement);
  setInterval(report, 1000);
}

const hm = (s) => `${Math.floor(s / 3600)}h ${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}m`;

// Escape stored text (app names, dates, hostnames) before it goes into innerHTML.
const esc = (s) => String(s ?? "").replace(/[&<>"']/g, (c) =>
  ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c]);
