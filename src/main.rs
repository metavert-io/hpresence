//! hpresence — a Hearthloom presence sampler, and the reference
//! third-party ember. It proves the whole sidecar contract with ONE
//! dependency (serde_json): a hand-rolled loopback HTTP client, the
//! plugin key file, self-storage KV and files, and the ingest scope.
//!
//! What it records, and nothing more:
//! - the frontmost app's name (`lsappinfo`, no permission prompt);
//! - whether you're idle: the HID idle time (`ioreg`) against the
//!   sample interval;
//! - only for browsers listed in `track_browsers` (none by default): the
//!   active tab's website DOMAIN, read with AppleScript (macOS asks once
//!   per browser). Only http(s) pages count, and the path, query,
//!   fragment and page title are dropped inside `site_host`. Which
//!   private windows can be skipped is spelled out on `browser_script`;
//! - the machine's short hostname (`hostname -s`), which appears in the
//!   day's episode title, its Device entity, KV keys and file paths;
//! - active seconds per local hour of the day.
//!
//! Never screenshots, audio, keystrokes, window titles or page titles.
//! One `presence_day` episode per machine per day; live state, a 14-day
//! rollup and monthly history files live in plugin storage, which
//! `sync: mirror` copies to the user's other nodes that have hpresence
//! installed too (hosted nodes can't: `placement: local`).

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::process::Command;
use std::time::Duration;

use serde_json::{json, Value};

const DEFAULT_INTERVAL_SECS: u64 = 30;
const RECENT_DAYS: usize = 14;
/// Where the daemon listens when HEARTHLOOM_URL is unset.
const DEFAULT_URL: &str = "http://127.0.0.1:43278";
/// Set to `1` to let the sidecar talk to a daemon that is NOT on this
/// machine. Off by default: this client speaks no TLS, so the plugin key
/// and the presence data would cross the network in cleartext.
const ALLOW_REMOTE_ENV: &str = "HPRESENCE_ALLOW_REMOTE";

struct Client {
    /// The Host header: `host[:port]`, IPv6 in brackets.
    authority: String,
    /// Where requests may go: loopback addresses only, unless
    /// HPRESENCE_ALLOW_REMOTE=1 (see `daemon_addrs`).
    addrs: Vec<SocketAddr>,
    key_file: String,
}

impl Client {
    fn from_env() -> Result<Self, String> {
        let url = std::env::var("HEARTHLOOM_URL")
            .ok()
            .filter(|u| !u.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_URL.into());
        let (host, port) = parse_base_url(&url)?;
        let addrs = daemon_addrs(&host, port, env_flag(ALLOW_REMOTE_ENV))?;
        if addrs.iter().any(|a| !a.ip().is_loopback()) {
            eprintln!(
                "hpresence: {ALLOW_REMOTE_ENV} is set: sending the plugin key and presence \
                 data to {host}:{port} unencrypted"
            );
        }
        let bracketed = if host.contains(':') { format!("[{host}]") } else { host };
        let authority = if port == 80 { bracketed } else { format!("{bracketed}:{port}") };
        Ok(Client {
            authority,
            addrs,
            key_file: std::env::var("HEARTHLOOM_PLUGIN_KEY_FILE").unwrap_or_default(),
        })
    }

    fn key(&self) -> String {
        std::fs::read_to_string(&self.key_file)
            .map(|s| s.trim().to_string())
            .unwrap_or_default()
    }

    /// Minimal HTTP/1.1 to the daemon — no TLS, no keep-alive, no
    /// chunked bodies (the daemon answers with Content-Length).
    fn request(&self, method: &str, path: &str, body: Option<&str>) -> Result<(u16, String), String> {
        let mut stream = TcpStream::connect(&self.addrs[..]).map_err(|e| format!("connect: {e}"))?;
        stream.set_read_timeout(Some(Duration::from_secs(30))).ok();
        stream.set_write_timeout(Some(Duration::from_secs(30))).ok();
        let body = body.unwrap_or("");
        let req = format!(
            "{method} {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\n\
             Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            self.authority,
            self.key(),
            body.len(),
        );
        stream.write_all(req.as_bytes()).map_err(|e| format!("write: {e}"))?;
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).map_err(|e| format!("read: {e}"))?;
        let text = String::from_utf8_lossy(&raw);
        let status: u16 = text
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let payload = text
            .split_once("\r\n\r\n")
            .map(|(_, b)| b.to_string())
            .unwrap_or_default();
        Ok((status, payload))
    }

    fn kv_get(&self, k: &str) -> Option<Value> {
        let (status, body) = self.request("GET", &format!("/v1/plugins/self/kv/{k}"), None).ok()?;
        if status != 200 {
            return None;
        }
        serde_json::from_str::<Value>(&body)
            .ok()?
            .get("value")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str(s).ok())
    }

    fn kv_put(&self, k: &str, v: &Value) {
        let _ = self.request("PUT", &format!("/v1/plugins/self/kv/{k}"), Some(&v.to_string()));
    }

    fn file_get(&self, path: &str) -> Option<Value> {
        let (status, body) = self
            .request("GET", &format!("/v1/plugins/self/files/{path}"), None)
            .ok()?;
        (status == 200).then(|| serde_json::from_str(&body).ok()).flatten()
    }

    fn file_put(&self, path: &str, v: &Value) {
        let _ = self.request(
            "PUT",
            &format!("/v1/plugins/self/files/{path}"),
            Some(&v.to_string()),
        );
    }
}

/// `http://host[:port][/…]` → (host, port), an IPv6 host without its
/// brackets. Plain http only: the daemon is on loopback, and this client
/// speaks no TLS.
fn parse_base_url(url: &str) -> Result<(String, u16), String> {
    let bad = || format!("HEARTHLOOM_URL {url:?} isn't an http://host[:port] address");
    let rest = strip_prefix_ci(url.trim(), "http://").ok_or_else(bad)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    if authority.contains('@') {
        return Err(bad());
    }
    let (host, port) = match authority.strip_prefix('[') {
        Some(v6) => {
            let (host, tail) = v6.split_once(']').ok_or_else(bad)?;
            if tail.is_empty() {
                (host, None)
            } else {
                (host, Some(tail.strip_prefix(':').ok_or_else(bad)?))
            }
        }
        None => match authority.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (authority, None),
        },
    };
    let port = match port {
        Some(p) => p.parse::<u16>().map_err(|_| bad())?,
        None => 80,
    };
    if host.is_empty() || port == 0 {
        return Err(bad());
    }
    Ok((host.to_string(), port))
}

/// The daemon's socket addresses. Unless `allow_remote`, only loopback
/// ones are kept, and none left is an error: the plugin key and the data
/// never leave this machine because of a mistyped or planted
/// HEARTHLOOM_URL.
fn daemon_addrs(host: &str, port: u16, allow_remote: bool) -> Result<Vec<SocketAddr>, String> {
    let addrs: Vec<SocketAddr> = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("can't resolve HEARTHLOOM_URL host {host:?}: {e}"))?
        .filter(|a| allow_remote || a.ip().is_loopback())
        .collect();
    if addrs.is_empty() {
        return Err(format!(
            "refusing to send the plugin key and data to {host}:{port}, which isn't this machine \
             (set {ALLOW_REMOTE_ENV}=1 to allow a remote daemon; the traffic is not encrypted)"
        ));
    }
    Ok(addrs)
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
}

/// `s` without `prefix`, compared ASCII-case-insensitively.
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &s[prefix.len()..])
}

/// System idle seconds via IOKit's HIDIdleTime (nanoseconds).
fn idle_secs() -> u64 {
    let out = Command::new("ioreg")
        .args(["-c", "IOHIDSystem", "-d", "4"])
        .output();
    let Ok(out) = out else { return 0 };
    let text = String::from_utf8_lossy(&out.stdout);
    for line in text.lines() {
        if let Some(pos) = line.find("\"HIDIdleTime\"") {
            let tail = &line[pos..];
            let digits: String = tail.chars().filter(|c| c.is_ascii_digit()).collect();
            if let Ok(ns) = digits.trim_start_matches("HIDIdleTime").parse::<u128>() {
                return (ns / 1_000_000_000) as u64;
            }
        }
    }
    0
}

/// Frontmost app name via lsappinfo (no accessibility permission
/// needed — unlike System Events).
fn frontmost_app() -> Option<String> {
    let front = Command::new("lsappinfo").arg("front").output().ok()?;
    let asn = String::from_utf8_lossy(&front.stdout).trim().to_string();
    if asn.is_empty() {
        return None;
    }
    let info = Command::new("lsappinfo")
        .args(["info", "-only", "name", &asn])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&info.stdout);
    // `"LSDisplayName"="Safari"` (quotes optional on older macOS)
    let value = text.split('=').nth(1)?.trim().trim_matches('"').to_string();
    (!value.is_empty()).then_some(value)
}

/// Browsers whose AppleScript windows carry Chromium's `mode` property
/// ("normal" or "incognito", per each app's scripting dictionary).
const CHROMIUM: &[&str] = &["Google Chrome", "Brave Browser", "Microsoft Edge", "Arc", "Chromium"];

/// The AppleScript that reads the frontmost browser's active-tab URL,
/// or None for a browser we can't read (Firefox has no scripting URL
/// surface; it, and any browser not listed here, counts as an app only).
///
/// Chromium family, FAIL-CLOSED: a URL comes back only when the front
/// window reports its mode as "normal". Incognito and private windows,
/// any other mode, and a browser whose windows have no `mode` (the
/// script then errors) all yield nothing. `set w to front window`
/// resolves the window once, so the mode check and the URL read address
/// the same window.
///
/// Safari, NOT fail-closed: its scripting dictionary has no way to tell
/// a private window from a normal one, so a private window is read like
/// any other. Safari is off unless the user lists it, and the setting's
/// help text says so.
fn browser_script(app: &str) -> Option<String> {
    if CHROMIUM.contains(&app) {
        Some(format!(
            "tell application \"{app}\"\n\
             if (count of windows) is 0 then return \"\"\n\
             set w to front window\n\
             if (mode of w) is not \"normal\" then return \"\"\n\
             return URL of active tab of w\n\
             end tell"
        ))
    } else if app == "Safari" {
        Some(
            "tell application \"Safari\"\n\
             if (count of documents) is 0 then return \"\"\n\
             return URL of front document\n\
             end tell"
                .to_string(),
        )
    } else {
        None
    }
}

/// The frontmost browser's active-tab URL. Any failure (Automation
/// permission denied, no window, a window the script won't read) is None.
fn browser_url(app: &str) -> Option<String> {
    let script = browser_script(app)?;
    let out = Command::new("osascript").args(["-e", &script]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!url.is_empty()).then_some(url)
}

/// The host of an http(s) URL: lowercased, without userinfo, port,
/// trailing dot or a leading "www."; an IPv6 literal loses its brackets.
/// Any other scheme (about:, chrome://, file:, data:…) is None, so
/// browser-internal pages never become pseudo-sites. The path, query,
/// fragment and title never leave this function.
fn site_host(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = strip_prefix_ci(url, "https://").or_else(|| strip_prefix_ci(url, "http://"))?;
    let authority = rest.split(['/', '\\', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    let host = match host_port.strip_prefix('[') {
        Some(v6) => v6.split(']').next()?,
        None => host_port.split(':').next()?,
    };
    let host = host.trim_end_matches('.').to_lowercase();
    let host = host.trim_start_matches("www.");
    (!host.is_empty()).then(|| host.to_string())
}

/// A host's registrable domain: its last two labels, or three under
/// common second-level suffixes (co.uk…). IP literals and short hosts
/// stay whole.
fn registrable_domain(host: &str) -> String {
    if host.parse::<IpAddr>().is_ok() {
        return host.to_string();
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() <= 2 {
        return host.to_string();
    }
    const SLD: &[&str] = &["co.uk", "org.uk", "ac.uk", "com.au", "co.jp", "co.nz", "com.br"];
    let tail2 = labels[labels.len() - 2..].join(".");
    let n = if SLD.contains(&tail2.as_str()) { 3 } else { 2 };
    labels[labels.len() - n..].join(".")
}

/// URL → the website domain hpresence records: the registrable domain of
/// an http(s) page, or None for any other page and for a host the user
/// excluded. Exclusions match the FULL host, so "example.com" covers its
/// subdomains and "mail.example.com" excludes only that subdomain.
fn domain_of(url: &str, excluded: &[String]) -> Option<String> {
    let host = site_host(url)?;
    let is_excluded = excluded
        .iter()
        .any(|x| host == *x || host.ends_with(&format!(".{x}")));
    (!is_excluded).then(|| registrable_domain(&host))
}

/// An exclusion as typed ("example.com", "www.Example.com",
/// "https://example.com/page", "*.example.com") → the bare host.
fn exclusion_host(entry: &str) -> Option<String> {
    let entry = entry.trim().trim_start_matches("*.");
    if entry.contains("://") {
        site_host(entry)
    } else {
        site_host(&format!("http://{entry}"))
    }
}

fn local_date() -> String {
    // date +%F respects the local timezone without a tz database dep
    Command::new("date")
        .arg("+%F")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}

fn local_hour() -> String {
    Command::new("date")
        .arg("+%H")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "00".into())
}

fn hostname() -> String {
    Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "this-machine".into())
}

fn tz_offset_minutes() -> i64 {
    Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let (sign, rest) = s.split_at(1);
            let h: i64 = rest.get(0..2)?.parse().ok()?;
            let m: i64 = rest.get(2..4)?.parse().ok()?;
            Some(if sign == "-" { -(h * 60 + m) } else { h * 60 + m })
        })
        .unwrap_or(0)
}

struct Day {
    date: String,
    host: String,
    per_app: BTreeMap<String, u64>,
    /// active seconds per local hour "00".."23" — the punchcard's fuel
    per_hour: BTreeMap<String, u64>,
    /// active seconds per website (registrable domain, merged across
    /// browsers) — only for browsers the user opted into tracking
    per_domain: BTreeMap<String, u64>,
}

impl Day {
    fn new(date: String, host: &str) -> Day {
        Day {
            date,
            host: host.to_string(),
            per_app: BTreeMap::new(),
            per_hour: BTreeMap::new(),
            per_domain: BTreeMap::new(),
        }
    }

    fn total(&self) -> u64 {
        self.per_app.values().sum()
    }

    fn to_state(&self) -> Value {
        json!({
            "date": self.date,
            "host": self.host,
            "per_app": self.per_app,
            "per_hour": self.per_hour,
            "per_domain": self.per_domain,
            "total_active_secs": self.total(),
        })
    }

    /// Tolerant of missing fields: state written before 0.6.0 has no
    /// per_domain.
    fn from_state(v: &Value) -> Option<Day> {
        Some(Day {
            date: v.get("date")?.as_str()?.to_string(),
            host: v.get("host").and_then(|h| h.as_str()).unwrap_or_default().to_string(),
            per_app: v
                .get("per_app")?
                .as_object()?
                .iter()
                .map(|(k, n)| (k.clone(), n.as_u64().unwrap_or(0)))
                .collect(),
            per_hour: v
                .get("per_hour")
                .and_then(|h| h.as_object())
                .map(|m| m.iter().map(|(k, n)| (k.clone(), n.as_u64().unwrap_or(0))).collect())
                .unwrap_or_default(),
            per_domain: v
                .get("per_domain")
                .and_then(|h| h.as_object())
                .map(|m| m.iter().map(|(k, n)| (k.clone(), n.as_u64().unwrap_or(0))).collect())
                .unwrap_or_default(),
        })
    }
}

fn fmt_hm(secs: u64) -> String {
    format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
}

/// Mint the day's episode. Deterministic source_ref → idempotent; the
/// daemon stamps the actor from our key regardless of what we claim.
fn flush_day(client: &Client, day: &Day) {
    if day.total() == 0 || day.date.is_empty() {
        return;
    }
    let mut apps: Vec<(&String, &u64)> = day.per_app.iter().collect();
    apps.sort_by(|a, b| b.1.cmp(a.1));
    let mut body = format!(
        "Active on {} for {} across {} apps.\n\n| app | time |\n|---|---|\n",
        day.host,
        fmt_hm(day.total()),
        apps.len(),
    );
    for (app, secs) in &apps {
        body.push_str(&format!("| {app} | {} |\n", fmt_hm(**secs)));
    }
    let batch = json!({
        "actor": "plugin:hpresence",
        "episodes": [{
            "source": "hpresence",
            // host-scoped: every machine's presence is its own fact,
            // idempotent per (machine, day) — ready for many computers
            "source_ref": format!("presence:{}:{}", day.host, day.date),
            "kind": "presence_day",
            "title": format!("Presence — {} on {}: {} active", day.date, day.host, fmt_hm(day.total())),
            "body": body,
            "event_time": {
                "utc": format!("{}T12:00:00Z", day.date),
                "tz_offset_minutes": tz_offset_minutes(),
                "uncertainty": "day",
            },
            "trust": "self",
            "scope": null,
            "entities": [{
                "entity_type": "Device",
                "name": day.host,
                "attributes": [],
                "role": "device",
            }],
            "edges": [],
        }],
    });
    match client.request("POST", "/v1/ingest", Some(&batch.to_string())) {
        Ok((200, _)) => eprintln!("hpresence: flushed {} on {} ({})", day.date, day.host, fmt_hm(day.total())),
        Ok((status, body)) => eprintln!("hpresence: ingest answered {status}: {body}"),
        Err(e) => eprintln!("hpresence: ingest failed: {e}"),
    }
}

/// Merge a day into its monthly history file — the heatmap's archive
/// (history/<host>/<YYYY-MM>.json = {"days": {date: {per_app, per_hour,
/// per_domain, total}}}). Whole-file rewrite; months are small.
fn write_history(client: &Client, day: &Day) {
    if day.date.len() < 7 || day.total() == 0 {
        return;
    }
    let month = &day.date[..7];
    let path = format!("history/{}/{month}.json", day.host);
    let mut doc = client.file_get(&path).unwrap_or_else(|| json!({"days": {}}));
    doc["days"][&day.date] = json!({
        "per_app": day.per_app,
        "per_hour": day.per_hour,
        "per_domain": day.per_domain,
        "total": day.total(),
    });
    client.file_put(&path, &doc);
}

/// Roll the finished day into the 14-day "recent" KV blob the UI reads.
fn update_recent(client: &Client, day: &Day) {
    let mut recent = client
        .kv_get("recent")
        .and_then(|v| v.as_array().cloned())
        .unwrap_or_default();
    recent.retain(|d| !(d.get("date").and_then(|x| x.as_str()) == Some(day.date.as_str())
        && d.get("host").and_then(|x| x.as_str()).unwrap_or("") == day.host));
    recent.push(day.to_state());
    recent.sort_by(|a, b| {
        a.get("date").and_then(|x| x.as_str()).unwrap_or("")
            .cmp(b.get("date").and_then(|x| x.as_str()).unwrap_or(""))
    });
    let hosts = recent.iter()
        .filter_map(|d| d.get("host").and_then(|h| h.as_str()))
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        .max(1);
    if recent.len() > RECENT_DAYS * hosts {
        let cut = recent.len() - RECENT_DAYS * hosts;
        recent.drain(..cut);
    }
    client.kv_put("recent", &Value::Array(recent));
}

fn main() {
    let client = match Client::from_env() {
        Ok(client) => client,
        Err(e) => {
            eprintln!("hpresence: {e}");
            std::process::exit(2);
        }
    };
    let host = hostname();
    let state_key = format!("state:{host}");
    eprintln!("hpresence: sampling every {DEFAULT_INTERVAL_SECS}s on {host} (config in plugin KV)");

    // register this machine in the hosts index (the UI reads it)
    {
        let mut hosts = client
            .kv_get("hosts")
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();
        if !hosts.iter().any(|h| h.as_str() == Some(host.as_str())) {
            hosts.push(json!(host));
            client.kv_put("hosts", &Value::Array(hosts));
        }
    }

    // Crash/restart recovery: pick up the day we were mid-way through;
    // a day left over from before a restart is flushed by the loop's
    // first date check.
    let mut day = client
        .kv_get(&state_key)
        .and_then(|v| Day::from_state(&v))
        .map(|mut d| {
            d.host = host.clone(); // state:<host> belongs to this machine
            d
        })
        .unwrap_or_else(|| Day::new(local_date(), &host));
    let mut interval = DEFAULT_INTERVAL_SECS;
    let mut ignore: Vec<String> = vec!["loginwindow".into(), "ScreenSaverEngine".into()];
    // website capture is opt-in: no browser is read until the user lists it
    let mut track_browsers: Vec<String> = Vec::new();
    let mut exclude_domains: Vec<String> = Vec::new();
    let mut ticks = 0u64;

    loop {
        // config re-read every ~5 minutes (and at boot)
        if ticks % (300 / interval.max(1)).max(1) == 0 {
            if let Some(cfg) = client.kv_get("config") {
                if let Some(n) = cfg.get("interval_secs").and_then(|v| v.as_u64()) {
                    interval = n.clamp(10, 300);
                }
                if let Some(list) = cfg.get("ignore").and_then(|v| v.as_array()) {
                    ignore = list.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                }
                if let Some(list) = cfg.get("track_browsers").and_then(|v| v.as_array()) {
                    track_browsers = list.iter().filter_map(|v| v.as_str().map(String::from)).collect();
                }
                if let Some(list) = cfg.get("exclude_domains").and_then(|v| v.as_array()) {
                    exclude_domains = list
                        .iter()
                        .filter_map(|v| v.as_str().and_then(exclusion_host))
                        .collect();
                }
            }
        }
        ticks += 1;

        let today = local_date();
        if today != day.date && !today.is_empty() {
            flush_day(&client, &day);
            update_recent(&client, &day);
            write_history(&client, &day);
            day = Day::new(today, &host);
            client.kv_put(&state_key, &day.to_state());
        }

        if idle_secs() < interval {
            if let Some(app) = frontmost_app() {
                if !ignore.iter().any(|i| i.eq_ignore_ascii_case(&app)) {
                    // websites: time in a tracked browser also counts
                    // toward the active tab's DOMAIN (merged across
                    // browsers); excluded sites are never written
                    if track_browsers.iter().any(|b| b.eq_ignore_ascii_case(&app)) {
                        if let Some(domain) = browser_url(&app).and_then(|url| domain_of(&url, &exclude_domains)) {
                            *day.per_domain.entry(domain).or_insert(0) += interval;
                        }
                    }
                    *day.per_app.entry(app).or_insert(0) += interval;
                    *day.per_hour.entry(local_hour()).or_insert(0) += interval;
                }
            }
        }

        // persist live state (the widget reads it) every ~2 minutes
        if ticks % (120 / interval.max(1)).max(1) == 0 {
            client.kv_put(&state_key, &day.to_state());
            update_recent(&client, &day); // today's running total in the charts too
            write_history(&client, &day); // the heatmap sees today live
        }

        std::thread::sleep(Duration::from_secs(interval));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(url: &str) -> Option<String> {
        domain_of(url, &[])
    }

    #[test]
    fn domain_of_keeps_only_the_registrable_domain() {
        let cases = [
            ("https://www.example.com/path?q=secret#frag", "example.com"),
            ("http://news.example.com:8080/a/b", "example.com"),
            ("https://user:pass@docs.example.org/x", "example.org"),
            ("HTTPS://WWW.EXAMPLE.COM/", "example.com"),
            ("https://example.com./", "example.com"),
            ("https://example.com?q=1", "example.com"),
            ("https://example.com#top", "example.com"),
            ("  https://example.com/\n", "example.com"),
            ("https://www.bbc.co.uk/news", "bbc.co.uk"),
            ("https://shop.example.co.jp/item", "example.co.jp"),
            ("https://example.co.uk/", "example.co.uk"),
            ("http://localhost:3000/", "localhost"),
        ];
        for (url, want) in cases {
            assert_eq!(site(url).as_deref(), Some(want), "{url}");
        }
    }

    #[test]
    fn domain_of_skips_everything_but_http_and_https() {
        for url in [
            "about:blank",
            "about:newtab",
            "chrome://settings",
            "chrome://newtab/",
            "edge://flags",
            "brave://rewards",
            "arc://extensions",
            "chrome-extension://abcdefgh/popup.html",
            "file:///tmp/notes.txt",
            "data:text/html,hello",
            "javascript:void(0)",
            "blob:https://example.com/0f3c",
            "view-source:https://example.com/",
            "favorites://",
            "ftp://example.com/",
            "missing value",
            "example.com",
            "",
            "https://",
            "https:///path",
        ] {
            assert_eq!(site(url), None, "{url:?}");
        }
    }

    #[test]
    fn domain_of_keeps_ip_literals_whole() {
        assert_eq!(site("http://192.168.1.10:8080/admin").as_deref(), Some("192.168.1.10"));
        assert_eq!(site("http://127.0.0.1/").as_deref(), Some("127.0.0.1"));
        assert_eq!(site("http://[::1]:8080/").as_deref(), Some("::1"));
        assert_eq!(site("https://[2001:db8::1]/x").as_deref(), Some("2001:db8::1"));
    }

    #[test]
    fn exclusions_cover_a_site_and_its_subdomains() {
        let excluded: Vec<String> = [
            "example.com",
            "https://www.Private.org/page",
            "mail.example.net",
            "*.wiki.test",
            "   ",
        ]
        .iter()
        .filter_map(|e| exclusion_host(e))
        .collect();
        assert_eq!(excluded, ["example.com", "private.org", "mail.example.net", "wiki.test"]);
        for url in [
            "https://example.com/",
            "https://www.example.com/a",
            "https://a.b.example.com/",
            "https://private.org/",
            "https://mail.example.net/inbox",
            "https://en.wiki.test/page",
        ] {
            assert_eq!(domain_of(url, &excluded), None, "{url}");
        }
        assert_eq!(domain_of("https://notexample.com/", &excluded).as_deref(), Some("notexample.com"));
        // excluding one subdomain leaves its siblings recorded
        assert_eq!(domain_of("https://docs.example.net/", &excluded).as_deref(), Some("example.net"));
    }

    #[test]
    fn fmt_hm_shows_hours_and_padded_minutes() {
        assert_eq!(fmt_hm(0), "0h 00m");
        assert_eq!(fmt_hm(59), "0h 00m");
        assert_eq!(fmt_hm(60), "0h 01m");
        assert_eq!(fmt_hm(3_599), "0h 59m");
        assert_eq!(fmt_hm(3_600), "1h 00m");
        assert_eq!(fmt_hm(3_661), "1h 01m");
        assert_eq!(fmt_hm(86_399), "23h 59m");
        assert_eq!(fmt_hm(90_000), "25h 00m");
    }

    #[test]
    fn chromium_scripts_read_only_normal_windows() {
        for app in CHROMIUM {
            let script = browser_script(app).unwrap();
            let gate = script.find("if (mode of w) is not \"normal\" then return \"\"").unwrap();
            let read = script.find("return URL of active tab of w").unwrap();
            assert!(gate < read, "{app}: the URL must be read after the mode check");
            assert!(script.contains("set w to front window"), "{app}");
        }
        assert!(browser_script("Safari").is_some());
        assert_eq!(browser_script("Firefox"), None);
        assert_eq!(browser_script("Terminal"), None);
    }

    #[test]
    fn base_url_must_be_plain_http() {
        assert_eq!(parse_base_url("http://127.0.0.1:43278"), Ok(("127.0.0.1".into(), 43278)));
        assert_eq!(parse_base_url("http://127.0.0.1:43278/"), Ok(("127.0.0.1".into(), 43278)));
        assert_eq!(parse_base_url("HTTP://localhost"), Ok(("localhost".into(), 80)));
        assert_eq!(parse_base_url("http://[::1]:43278"), Ok(("::1".into(), 43278)));
        for url in [
            "https://127.0.0.1:43278",
            "127.0.0.1:43278",
            "http://user@127.0.0.1:43278",
            "http://127.0.0.1:port",
            "http://127.0.0.1:0",
            "http://:43278",
            "http://[::1",
        ] {
            assert!(parse_base_url(url).is_err(), "{url}");
        }
    }

    #[test]
    fn only_loopback_daemons_without_the_opt_in() {
        assert!(daemon_addrs("127.0.0.1", 43278, false).is_ok());
        assert!(daemon_addrs("::1", 43278, false).is_ok());
        let err = daemon_addrs("192.0.2.10", 43278, false).unwrap_err();
        assert!(err.contains(ALLOW_REMOTE_ENV), "{err}");
        assert!(daemon_addrs("192.0.2.10", 43278, true).is_ok());
    }

    #[test]
    fn day_state_from_older_versions_still_reads() {
        // 0.4/0.5 state: per_hour but no per_domain yet
        let old = json!({
            "date": "2026-07-22", "host": "studio",
            "per_app": {"Mail": 60, "Terminal": 120}, "per_hour": {"09": 180},
            "total_active_secs": 180,
        });
        let day = Day::from_state(&old).unwrap();
        assert_eq!(day.total(), 180);
        assert!(day.per_domain.is_empty());
        let again = Day::from_state(&day.to_state()).unwrap();
        assert_eq!((again.host.as_str(), again.per_app, again.per_hour), ("studio", day.per_app, day.per_hour));
    }
}
