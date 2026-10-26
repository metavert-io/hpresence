# hpresence: review guidance

hpresence is a Hearthloom ember for Apple-silicon Macs: a sampler that
records the frontmost app, idle state and, only for browsers the user lists,
website domains, and posts one summary a day to the Hearthloom daemon on the
same Mac. One crate (`src/main.rs`), the UI pages in `ui/`, the manifest in
`hearthember.json`. `README.md` ("Privacy") and `SECURITY.md` state what it
promises; every change must keep those promises true.

## Privacy
- Website capture stays opt-in per browser. Only the registrable domain of a
  tab's address is kept, reduced inside the sampler; paths, queries,
  fragments and page titles are never stored, and only `http`/`https` pages
  count.
- Chromium-family browsers are read only from a window that reports the mode
  "normal": fail closed when the mode can't be read. Firefox stays app-only.
- AppleScript is built only for browsers on the fixed `CHROMIUM` list or
  Safari. Never interpolate settings, URLs or page content into a script.
- Excluded sites are dropped before anything is written: never in the
  ember's KV, files, episodes or logs.
- A new data type, field or permission prompt needs matching text in
  README.md and SECURITY.md.

## Talking to Hearthloom
- Requests go only to loopback. A non-loopback `HEARTHLOOM_URL` is refused
  before the key is read, unless `HPRESENCE_ALLOW_REMOTE=1` is set.
- The key comes from the file the daemon wrote; never log it or put it in an
  error. The manifest requests `ingest` only: don't add scopes.
- Daily episodes use a deterministic ref, so a re-post is a no-op. Ignore
  unknown response fields, and treat a 404/405 as an older host: degrade,
  don't crash.
- Keep the dependency list at `serde_json`: no SDK, just the small loopback
  HTTP client in `src/main.rs`.

## UI pages (`ui/`)
They run in the dashboard's sandboxed iframe. Accept `postMessage` only from
the embedding dashboard window, and escape every stored string (app names,
hostnames, dates) before it reaches the DOM. No `alert`/`confirm`/`prompt`,
and no remote scripts, styles or fonts.

## Builds
`--locked`, `aarch64-apple-darwin` only, and keep build.sh's
`--remap-path-prefix`, which keeps home-directory paths out of the binary.
