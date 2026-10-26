# Security policy

## Reporting a vulnerability

Please report security problems privately, through GitHub: open this
repository's **Security** tab and choose **Report a vulnerability**. That
creates a private advisory that only the maintainers can see. Please do not
open a public issue, pull request or discussion about a vulnerability.

A useful report says what you found, how to reproduce it, which version or
commit you tested, and what an attacker could do with it. Fixes land on
`main` and ship in the next release; older releases are not patched
separately.

## What hpresence protects, and how

- **It talks only to the Hearthloom on this Mac.** The sidecar sends
  requests only to loopback addresses. A `HEARTHLOOM_URL` that points
  anywhere else is refused at startup, before the key is read or anything
  is sent, unless you set `HPRESENCE_ALLOW_REMOTE=1` yourself. The client
  speaks plain HTTP (the daemon is on loopback), so only use that switch
  on a network you trust.
- **It asks for one scope, `ingest`.** It can add memories to your record
  and use its own storage, but it can't read your record. It reads its key
  from the file the daemon wrote for it and never logs it.
- **No permission prompts for the basics.** The app name and idle time
  come from `lsappinfo` and `ioreg`. A browser is read only after you list
  it under "track websites in these browsers", through AppleScript, which
  macOS gates behind an Automation prompt for each browser.
- **Domains, not URLs.** A tab's address is reduced to its registrable
  domain inside the sampler: paths, queries, fragments and page titles are
  never stored, and only `http` and `https` pages count.
- **Private windows.** Chrome, Brave, Edge, Arc and Chromium are read only
  from a window that reports the mode "normal": incognito windows, and
  windows whose mode can't be read, are skipped. Safari has no way to
  report a private window, so if you turn Safari on, its private windows
  are recorded like any other.
- **Excluded sites are dropped before anything is written**, so they never
  reach your record, the ember's storage or your other nodes.
- **The pages are sandboxed.** The dashboard shows them in a sandboxed
  iframe with a short-lived token. They accept messages only from the
  dashboard window that embeds them, and escape stored text (app names,
  dates, hostnames) before putting it on the page.
- **Builds use exactly the dependencies in `Cargo.lock`** (`--locked`), and
  the binary carries no home-directory paths.

hpresence is a guest program on your Mac: Hearthloom's scopes protect your
record, but nothing makes a local binary safe for the machine it runs on.
Build it yourself, or install it only from a registry you trust.
