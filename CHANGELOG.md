# Changelog

## 0.7.1 — 2026-10-26

- **Website capture is opt-in.** `track_browsers` now defaults to no
  browsers (it was Google Chrome and Safari), both in the manifest and in
  the sampler's own fallback. A browser list you already saved in the
  settings form is kept.
- **Private windows, stated exactly.** Chrome, Brave, Edge, Arc and
  Chromium are read only when the front window reports the mode "normal";
  incognito, any other mode, or no mode at all means nothing is read. The
  mode check and the URL read now address the same window. Safari can't
  report private windows, and the setting's help text now says so instead
  of claiming they are skipped.
- **Only http(s) pages become sites.** `about:`, `chrome://`, `file:` and
  other non-web pages no longer turn into pseudo-domains, and IPv6
  addresses are parsed properly.
- **Exclusions match the whole host name**, so `mail.example.com` can be
  excluded on its own (before, a subdomain entry never matched anything),
  and entries typed as URLs or with `www.` or `*.` work too.
- **Loopback only.** The sidecar refuses to send its key or any data to a
  `HEARTHLOOM_URL` that isn't on this machine, unless
  `HPRESENCE_ALLOW_REMOTE=1` is set. `https://` and malformed URLs are
  refused instead of guessed at.
- The pages accept messages only from the dashboard window that embeds
  them, and escape dates and hostnames as well as app names.
- Manifest: `placement: "local"` replaces the legacy `localOnly`, the
  unused `kind` field is gone, and the description says exactly what is
  recorded. Requires Hearthloom 0.5.71 or later.
- Removed the one-time migrations for 0.1.x and 0.2.x data (the hostless
  `state` key and the 0.3.0 history backfill). Data written by 0.3.0 and
  later reads as before; an install upgrading straight from 0.1.x or
  0.2.x no longer carries over a 0.1.x day in progress, or the days before
  0.3.0 into the presence map.
- Builds with `--locked` and without home-directory paths in the binary.
  Adds `rust-toolchain.toml`, a minimum Rust version (1.78), unit tests,
  a LICENSE, SECURITY.md and this changelog.

## 0.7.0 — 2026-07-23

- The pages report their content height, so the dashboard sizes each one
  to fit and it scrolls with the dashboard instead of inside a frame.
- Requires Hearthloom 0.5.25 or later.

## 0.6.0 — 2026-07-22

- Websites, separate from apps: time in a tracked browser also counts
  toward the active tab's domain, merged across browsers. Chromium-family
  browsers and Safari are read through AppleScript (incognito windows
  skipped in the Chromium family); Firefox counts as an app only, as it
  offers no scripting access to its tabs. Which browsers are tracked is a
  setting (by default Google Chrome and Safari).
- An exclusion list: sites on it are never written anywhere.
- A websites treemap on the presence map.
- Settings moved into the manifest's declarative `config` list, which the
  dashboard renders on the ember's Integrations card; the settings page is
  gone.

## 0.5.0 — 2026-07-22

- A "View presence data" link on the ember's Integrations card replaces
  the top-navigation entry.
- The App time chart is gone; the presence map covers it.
- Treemap tooltips appear instantly.

## 0.4.0 — 2026-07-22

- An aggregate treemap of time per app replaces the app-by-day grid.
- Chart text is styled inside the sandbox (unstyled SVG text was black).

## 0.3.0 — 2026-07-21

- A Presence heatmap page under Visualize: minutes per app by day and an
  hour × weekday punchcard, for the window picked with the dashboard's
  dial.
- Active time per local hour, recorded alongside time per app.
- Monthly history files (`history/<host>/<YYYY-MM>.json`, mirrored),
  with a one-time backfill from the 14-day rollup.

## 0.2.0 — 2026-07-21

- Presence per machine, ready for several Macs: the state key is
  `state:<host>`, the episode ref `presence:<host>:<date>`, titles and
  bodies name the machine, each day carries a Device entity, the recent
  rollup is kept per host, and a hosts index lets the pages add up today
  across machines.
- `runner: per-node`: every Mac samples itself, since presence on two
  machines is two facts, not duplicates.
- State from 0.1.x, which had no host, is adopted on first start.

## 0.1.0 — 2026-07-21

- First release: the frontmost app and idle state, sampled every 30
  seconds and folded into one `presence_day` episode a day, with live
  state and a 14-day rollup in plugin storage. A Presence page, a "screen
  time today" Desk widget, an App time chart and a settings page.
