# hpresence

A presence ember for [Hearthloom](https://github.com/metavert-io/hearthloom),
and the reference third-party ember: its own repo and dependencies, talking
to the Hearthloom daemon only through its loopback HTTP API, with a scoped
key the user granted.

## What it records

Every 30 seconds by default, on each Mac where it is installed:

- the **frontmost app's name**;
- your **idle state**: a sample counts as active only if the keyboard,
  mouse or trackpad was used within the sample interval (macOS's HID idle
  time);
- **website domains, only for browsers you turn on** (none by default):
  the active tab's domain, such as `example.com`, never its path, query or
  page title;
- the machine's **short hostname** (`hostname -s`), which appears in
  episode titles, the Device entity, KV keys and file paths, so each Mac's
  presence stays separate;
- **hour-of-day buckets**: active time per local hour.

It never records screenshots, audio, keystrokes, window titles, page
titles or full addresses.

What it writes: one `presence_day` episode per Mac per day into your
Hearthloom record (time per app, with a title like "Presence — 2026-07-22
on studio: 6h 12m active"), plus live state, a 14-day rollup and monthly
history files in the ember's own storage.

- The episodes replicate to your paired nodes, your always-on node
  included, like the rest of your record.
- The ember's storage is `"sync": "mirror"`: Hearthloom copies it to your
  always-on node and other paired nodes, but a node accepts it only if
  hpresence is installed there too. Hosted nodes (any node that listens
  beyond loopback) can't install hpresence (`placement: local`), so a
  hosted always-on node keeps the episodes but not this storage.
- Both are included in Hearthloom exports.

## What it adds to the dashboard

- **View presence data**: a link on hpresence's card in ☰ → Integrations →
  embers, showing today's time per app and the recent days.
- **Presence map**, under **Visualize**: treemaps of time per app and per
  website and an hour × weekday punchcard, for the window you pick with the
  dial.
- **screen time today**: a widget on the Desk.
- **Settings**: the config form on hpresence's Integrations card
  (**configure…**).

## Requirements

- An Apple Silicon Mac. The sidecar is built for `aarch64-apple-darwin`
  only; Intel Macs are not supported.
- Rust stable, 1.78 or later, to build it.
- Hearthloom 0.5.71 or later, running on the same Mac.

## Install

### 1. Build it yourself

```bash
git clone https://github.com/metavert-io/hpresence
cd hpresence
./build.sh                          # → dist/, the installable package
hearthloom ember validate ./dist    # check it with the daemon's own parser
```

Then, in the dashboard, open **☰ → Integrations → embers → "install one
you built yourself"**, enter the path to `dist/` and choose **review…**.
You see what it asks for (just `ingest`: it can add memories and nothing
else) before anything is granted. To upload a zip instead,
`hearthloom ember pack ./dist` makes `hpresence-<version>.zip`.

Self-built installs are labelled *unreviewed*: you are trusting your own
compiler instead of a registry's signatures, which is the point of this
path.

### 2. From a registry

hpresence can also come from the registry your node is configured with
(`HEARTHLOOM_REGISTRY_URL`, or `--registry <url>` for the CLI; Hearthlaunch
is one compatible registry). In the dashboard it appears under **☰ →
Integrations → embers → "available from the hearth"** (**add to your
hearth**, review, allow); from the shell:

```bash
hearthloom ember install hpresence
```

Your node checks the package's sha256, the publisher's Ed25519 release
signature and, when present, the registry's counter-signature before
anything runs. Afterwards:

```bash
hearthloom ember info hpresence      # versions, scopes, review status
hearthloom ember update              # pick up new versions, verified
hearthloom ember logs hpresence -f   # watch the sampler
```

## Settings

| setting | default | what it does |
|---|---|---|
| sample every (seconds) | 30 | how often presence is checked (10–300) |
| ignored apps | loginwindow, ScreenSaverEngine | apps that are never counted |
| track websites in these browsers | none | browsers whose active-tab domain is recorded (see Privacy) |
| never record these sites | none | sites never written anywhere; a domain covers its subdomains |

Settings are kept in the ember's `config` KV, which mirrors with the rest
of its storage; the sampler re-reads them every five minutes.

## Privacy

- **Website capture is opt-in.** No browser is read until you list it
  under "track websites in these browsers". The first time hpresence reads
  a browser, macOS asks whether hpresence may control it (an Automation
  prompt), once per browser; if you decline, that browser counts as an app
  only. The app name and idle time need no permission.
- **Domains only.** The active tab's address is reduced to its domain
  (`news.example.co.uk` becomes `example.co.uk`) inside the sampler; the
  path, query, fragment and page title are dropped before anything is
  stored. Only `http` and `https` pages count, so `about:`, `chrome://` and
  `file:` pages are never recorded.
- **Private windows.**
  - Google Chrome, Brave Browser, Microsoft Edge, Arc and Chromium: a site
    is read only from a window that reports itself as a normal window.
    Incognito and private windows are skipped, and so is any window whose
    mode the browser can't report.
  - Safari can't report whether a window is private, so its private
    windows would be recorded like any other. Add Safari only if you don't
    browse privately in it.
  - Firefox offers no scripting access to its tabs, so it always counts as
    an app only.
- **Excluded sites** are never written anywhere, not even into the vault.
  Time on them still counts toward the browser app.
- **Your data stays in your Hearthloom**: episodes join your record and
  replicate to your paired nodes, the ember's storage is mirrored to your
  nodes that have hpresence installed, and both are included in exports
  (see [What it records](#what-it-records)).
- **Loopback only.** The sidecar refuses to send its key or any data to a
  `HEARTHLOOM_URL` that isn't on this machine, because it speaks plain
  HTTP. If you deliberately run it against a daemon elsewhere, set
  `HPRESENCE_ALLOW_REMOTE=1` in its environment, and only on a network you
  trust.
- **One sampler per Mac.** `runner: per-node` means every Mac with
  hpresence installed runs its own sampler and records its own presence.
  `placement: local` keeps it off hosted nodes: it never installs or runs
  there.

## How it works (for ember authors)

This repo doubles as a worked example of the ember contract; the full guide
is [the plugin guide](https://github.com/metavert-io/hearthloom/blob/main/docs/plugin-guide.md)
in the Hearthloom repo.

| surface | where to look |
|---|---|
| manifest (scopes, placement, runner, sync, UI hooks, settings form) | `hearthember.json` |
| dependency-free loopback HTTP client (no SDK) | `Client` in `src/main.rs` |
| daily episode with a deterministic, idempotent ref | `flush_day()` |
| self-storage: KV (state, config, 14-day rollup) and files (monthly history) | `kv_get`/`kv_put`, `write_history()` |
| sandbox handshake for UI pages | `ui/hearth.js` |
| card page, Visualize page, Desk widget | `ui/index.html`, `ui/heat.html`, `ui/widget.html` |

To start your own ember from this one, change `name` and `publisher` in
`hearthember.json` (the name is your ember's identity and storage namespace
on every node) and request only the scopes you need. `cargo test` runs the
unit tests.

## Security

To report a vulnerability, and for what hpresence does to protect your
data, see [SECURITY.md](SECURITY.md).

## Related

Also from the Hearthloom family: [HearthFS](https://github.com/metavert-io/hearthfs) and [Hearthloom for iOS](https://github.com/metavert-io/hearthloom-ios).

## License

MIT © Metavert LLC. See [LICENSE](LICENSE).
