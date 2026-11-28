//! The cswap CLI as typed calls (port of `src/main/cswap/driver.ts`): one `run` that logs
//! every invocation, the mutation queue, and the parsers for the two verbs without --json.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use cswap_core::*;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};

use crate::proc::{self, RawOutcome, Spec};
use crate::time::now_iso;

/// Size cap of the LOG page, and of each stream kept per entry (0.1.23's numbers).
pub const LOG_CAP: usize = 300;
pub const LOG_STREAM_CAP: usize = 20_000;
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

type Job = Pin<Box<dyn Future<Output = ()> + Send>>;

/// Environment every cswap call gets: no colour codes in what we parse, UTF-8 output on
/// Windows consoles (cp1252 would choke on `→`), and no update nag on stderr.
pub fn base_env() -> Vec<(String, String)> {
    [("NO_COLOR", "1"), ("PYTHONIOENCODING", "utf-8"), ("PYTHONUTF8", "1"), ("CLAUDE_SWAP_NO_UPDATE_CHECK", "1")]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

pub struct RunOptions<'a> {
    pub stdin: Option<&'a str>,
    pub timeout: Duration,
}

impl Default for RunOptions<'_> {
    fn default() -> Self {
        Self { stdin: None, timeout: DEFAULT_TIMEOUT }
    }
}

fn secs(s: u64) -> RunOptions<'static> {
    RunOptions { stdin: None, timeout: Duration::from_secs(s) }
}

pub struct Cli {
    bin: RwLock<Option<PathBuf>>,
    /// Extra environment for every child (tests point the fake cswap at its state file).
    extra_env: RwLock<Vec<(String, String)>>,
    log: Mutex<Vec<CommandLogEntry>>,
    seq: AtomicU64,
    queue: mpsc::UnboundedSender<Job>,
    sink: Option<EventSink>,
}

impl Cli {
    /// Spawns the queue worker on `rt`.
    pub fn new(rt: &Handle, sink: Option<EventSink>, extra_env: Vec<(String, String)>) -> Arc<Cli> {
        let (tx, mut rx) = mpsc::unbounded_channel::<Job>();
        // One worker, jobs in the order they were queued: two clicks never race inside
        // cswap's own locks, and the second sees the first one's result.
        rt.spawn(async move {
            while let Some(job) = rx.recv().await {
                job.await;
            }
        });
        Arc::new(Cli {
            bin: RwLock::new(None),
            extra_env: RwLock::new(extra_env),
            log: Mutex::new(Vec::new()),
            seq: AtomicU64::new(0),
            queue: tx,
            sink,
        })
    }

    pub fn set_binary(&self, bin: Option<PathBuf>) {
        *self.bin.write().unwrap_or_else(|e| e.into_inner()) = bin;
    }
    pub fn binary(&self) -> Option<PathBuf> {
        self.bin.read().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn env(&self) -> Vec<(String, String)> {
        let mut env = base_env();
        env.extend(self.extra_env.read().unwrap_or_else(|e| e.into_inner()).iter().cloned());
        env
    }
    #[cfg(test)]
    pub fn set_env(&self, key: &str, value: Option<&str>) {
        let mut e = self.extra_env.write().unwrap_or_else(|e| e.into_inner());
        e.retain(|(k, _)| k != key);
        if let Some(v) = value {
            e.push((key.to_owned(), v.to_owned()));
        }
    }
    pub fn log(&self) -> Vec<CommandLogEntry> {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    pub fn clear_log(&self) {
        self.log.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    /// Queues `fut` behind every mutation queued before it. Must be called synchronously
    /// in call order (it is: the `Cswap` methods queue before returning).
    pub fn serial<T: Send + 'static>(
        &self,
        fut: impl Future<Output = T> + Send + 'static,
    ) -> impl Future<Output = Option<T>> + Send + 'static {
        let (tx, rx) = oneshot::channel();
        let _ = self.queue.send(Box::pin(async move {
            let _ = tx.send(fut.await);
        }));
        async move { rx.await.ok() }
    }

    /// Raw invocation. Redacts nothing on purpose: tokens never travel as arguments
    /// (`add-token -` reads them from stdin), and stdin is never logged.
    pub async fn run(&self, args: &[&str], opts: RunOptions<'_>) -> RawOutcome {
        let args: Vec<String> = args.iter().map(|s| (*s).to_owned()).collect();
        let out = match self.binary() {
            None => RawOutcome { spawn_error: Some("cswap binary not configured".into()), ..Default::default() },
            Some(bin) => {
                let env = self.env();
                proc::run(Spec {
                    bin: &bin,
                    args: &args,
                    env: &env,
                    stdin: opts.stdin,
                    timeout: opts.timeout,
                    cwd: None,
                })
                .await
            }
        };
        self.record(args, &out);
        out
    }

    fn record(&self, args: Vec<String>, out: &RawOutcome) {
        let cap = |s: &str| (!s.is_empty()).then(|| s.chars().take(LOG_STREAM_CAP).collect::<String>());
        let entry = CommandLogEntry {
            id: self.seq.fetch_add(1, Ordering::Relaxed) + 1,
            at: now_iso(),
            args,
            exit_code: out.exit_code,
            duration_ms: out.duration_ms,
            ok: out.spawn_error.is_none() && out.exit_code == Some(0),
            stdout: cap(&out.stdout),
            stderr: cap(&out.stderr),
            error: out.spawn_error.clone(),
        };
        {
            let mut log = self.log.lock().unwrap_or_else(|e| e.into_inner());
            log.push(entry.clone());
            if log.len() > LOG_CAP {
                let over = log.len() - LOG_CAP;
                log.drain(..over);
            }
        }
        if let Some(s) = &self.sink {
            s.send(Event::CommandLog(entry));
        }
    }

    // ---- typed commands ----------------------------------------------------

    pub async fn list(&self) -> CsResult<ListPayload> {
        parse_json(&self.run(&["list", "--json"], secs(120)).await)
    }
    pub async fn status(&self) -> CsResult<StatusPayload> {
        parse_json(&self.run(&["status", "--json"], secs(60)).await)
    }
    pub async fn switch_to(&self, target: &str, force: bool) -> CsResult<SwitchPayload> {
        let mut args = vec!["switch", target, "--json"];
        if force {
            args.push("--force");
        }
        parse_json(&self.run(&args, secs(120)).await)
    }
    pub async fn rotate(&self, strategy: Option<RotateStrategy>) -> CsResult<SwitchPayload> {
        let mut args = vec!["switch", "--json"];
        match strategy {
            Some(RotateStrategy::Best) => args.extend(["--strategy", "best"]),
            Some(RotateStrategy::NextAvailable) => args.extend(["--strategy", "next-available"]),
            None => {}
        }
        parse_json(&self.run(&args, secs(120)).await)
    }
    pub async fn add_account(&self, o: &AddAccountOptions) -> CsResult<PlainOutput> {
        let slot = o.slot.map(|s| s.to_string());
        let mut args = vec!["add"];
        if let Some(s) = &slot {
            args.extend(["--slot", s]);
        }
        if let Some(a) = o.alias.as_deref().filter(|a| !a.is_empty()) {
            args.extend(["--alias", a]);
        }
        // `add --slot N` asks "Overwrite slot N? [y/N]" when N is taken; answer on stdin.
        let stdin = if o.overwrite { "y\n" } else { "n\n" };
        plain(&self.run(&args, RunOptions { stdin: Some(stdin), timeout: DEFAULT_TIMEOUT }).await)
    }
    pub async fn add_token(&self, o: &AddTokenOptions) -> CsResult<PlainOutput> {
        let slot = o.slot.map(|s| s.to_string());
        let mut args = vec!["add-token", "-"];
        if let Some(e) = o.email.as_deref().filter(|e| !e.is_empty()) {
            args.extend(["--email", e]);
        }
        if let Some(s) = &slot {
            args.extend(["--slot", s]);
        }
        // The token goes on stdin line 1 (never argv), the overwrite answer on line 2.
        let stdin = format!("{}\n{}\n", o.token.trim(), if o.overwrite { "y" } else { "n" });
        plain(&self.run(&args, RunOptions { stdin: Some(&stdin), timeout: DEFAULT_TIMEOUT }).await)
    }
    pub async fn remove_account(&self, target: &str) -> CsResult<PlainOutput> {
        // The UI confirms first; cswap's own "[y/N]" prompt is answered on stdin.
        plain(&self.run(&["remove", target], RunOptions { stdin: Some("y\n"), timeout: DEFAULT_TIMEOUT }).await)
    }
    pub async fn set_disabled(&self, target: &str, disabled: bool) -> CsResult<PlainOutput> {
        plain(&self.run(&[if disabled { "disable" } else { "enable" }, target], RunOptions::default()).await)
    }
    pub async fn set_alias(&self, target: &str, alias: Option<&str>) -> CsResult<PlainOutput> {
        let args = match alias.filter(|a| !a.is_empty()) {
            Some(a) => vec!["alias", target, a],
            None => vec!["alias", target, "--unset"],
        };
        plain(&self.run(&args, RunOptions::default()).await)
    }
    pub async fn move_account(&self, target: &str, slot: u32) -> CsResult<PlainOutput> {
        plain(&self.run(&["move", target, &slot.to_string()], RunOptions::default()).await)
    }
    pub async fn swap_accounts(&self, a: &str, b: &str) -> CsResult<PlainOutput> {
        plain(&self.run(&["swap", a, b], RunOptions::default()).await)
    }
    pub async fn map_directory(&self, target: &str, path: &str) -> CsResult<PlainOutput> {
        plain(&self.run(&["map", target, path], RunOptions::default()).await)
    }
    pub async fn unmap_directory(&self, path: &str) -> CsResult<PlainOutput> {
        plain(&self.run(&["unmap", path], RunOptions::default()).await)
    }
    pub async fn list_mappings_text(&self) -> CsResult<PlainOutput> {
        plain(&self.run(&["map"], RunOptions::default()).await)
    }
    pub async fn config(&self) -> CsResult<ConfigPayload> {
        parse_json(&self.run(&["config", "list", "--json"], RunOptions::default()).await)
    }
    pub async fn config_path(&self) -> Option<String> {
        let out = self.run(&["config", "path"], RunOptions::default()).await;
        let p = out.stdout.trim();
        (out.spawn_error.is_none() && out.exit_code == Some(0) && !p.is_empty()).then(|| p.to_owned())
    }
    pub async fn set_config(&self, key: &str, value: Option<&str>) -> CsResult<PlainOutput> {
        let args = match value {
            None => vec!["config", "unset", key],
            Some(v) => vec!["config", "set", key, v],
        };
        plain(&self.run(&args, RunOptions::default()).await)
    }
    pub async fn export_accounts(&self, path: &str, o: &ExportOptions) -> CsResult<PlainOutput> {
        let mut args = vec!["export", path];
        if let Some(a) = o.account.as_deref().filter(|a| !a.is_empty()) {
            args.extend(["--account", a]);
        }
        if o.full {
            args.push("--full");
        }
        plain(&self.run(&args, secs(60)).await)
    }
    pub async fn import_accounts(&self, path: &str, o: &ImportOptions) -> CsResult<PlainOutput> {
        let mut args = vec!["import", path];
        if o.force {
            args.push("--force");
        }
        plain(&self.run(&args, secs(120)).await)
    }
    pub async fn unclaimed_text(&self) -> CsResult<PlainOutput> {
        plain(&self.run(&["unclaimed"], RunOptions::default()).await)
    }
    pub async fn purge_unclaimed(&self, id: &str) -> CsResult<PlainOutput> {
        plain(&self.run(&["unclaimed", "--purge", id], RunOptions::default()).await)
    }
    pub async fn token_status(&self) -> CsResult<PlainOutput> {
        plain(&self.run(&["list", "--token-status"], secs(120)).await)
    }
    pub async fn upgrade(&self) -> CsResult<PlainOutput> {
        plain(&self.run(&["upgrade"], secs(300)).await)
    }
    /// `cswap auto --once`: exit 0 switched · 1 error · 2 nothing to do · 3 blocked.
    pub async fn auto_once(&self, dry_run: bool) -> CsResult<AutoOnceResult> {
        let mut args = vec!["auto", "--once", "--json"];
        if dry_run {
            args.push("--dry-run");
        }
        parse_auto_once(&self.run(&args, secs(180)).await)
    }
}

fn spawn_err(out: &RawOutcome) -> Option<CswapError> {
    out.spawn_error.as_ref().map(|m| CswapError::new("SpawnError", m.clone()))
}

/// `(stderr || stdout || "exit N").trim()`, as 0.1.23 built a CLI error message.
fn cli_message(out: &RawOutcome) -> String {
    let raw = if !out.stderr.is_empty() {
        out.stderr.clone()
    } else if !out.stdout.is_empty() {
        out.stdout.clone()
    } else {
        format!("exit {}", out.exit_code.map_or("null".to_owned(), |c| c.to_string()))
    };
    raw.trim().to_owned()
}

/// cswap's error envelope `{"error":{"type","message"}}` if the value is one.
fn envelope_error(v: &Value) -> Option<CswapError> {
    let e = v.get("error")?;
    if e.is_null() || e == &Value::Bool(false) {
        return None;
    }
    Some(serde_json::from_value::<CswapError>(e.clone()).unwrap_or_else(|_| CswapError::new("CliError", e.to_string())))
}

/// The JSON document starts at the first `{` (anything before it is noise such as a
/// prompt); an error envelope wins over the exit code.
pub fn parse_json<T: DeserializeOwned>(out: &RawOutcome) -> CsResult<T> {
    if let Some(e) = spawn_err(out) {
        return Err(e);
    }
    if let Some(start) = out.stdout.find('{')
        && let Ok(v) = serde_json::from_str::<Value>(&out.stdout[start..])
        && v.is_object()
    {
        if let Some(e) = envelope_error(&v) {
            return Err(e);
        }
        if out.exit_code == Some(0) {
            return serde_json::from_value(v).map_err(|e| CswapError::new("ParseError", e.to_string()));
        }
    }
    Err(CswapError::new("CliError", cli_message(out)))
}

pub fn plain(out: &RawOutcome) -> CsResult<PlainOutput> {
    if let Some(e) = spawn_err(out) {
        return Err(e);
    }
    if out.exit_code == Some(0) {
        return Ok(PlainOutput { stdout: out.stdout.clone(), stderr: out.stderr.clone(), exit_code: Some(0) });
    }
    let msg = cli_message(out);
    let msg = match msg.get(..6) {
        Some(p) if p.eq_ignore_ascii_case("error:") => msg[6..].trim_start(),
        _ => &msg,
    };
    Err(CswapError::new("CliError", msg.to_owned()))
}

pub fn parse_auto_once(out: &RawOutcome) -> CsResult<AutoOnceResult> {
    if let Some(e) = spawn_err(out) {
        return Err(e);
    }
    let mut events = Vec::new();
    for line in out.stdout.lines() {
        let t = line.trim();
        if !t.starts_with('{') {
            continue;
        }
        let Ok(v) = serde_json::from_str::<Value>(t) else { continue };
        if let Some(e) = envelope_error(&v) {
            return Err(e);
        }
        if let Ok(ev) = serde_json::from_value::<AutoEvent>(v) {
            events.push(ev);
        }
    }
    let outcome = match out.exit_code {
        Some(0) => AutoOutcome::Switched,
        Some(1) => AutoOutcome::Error,
        Some(2) => AutoOutcome::NoAction,
        Some(3) => AutoOutcome::Blocked,
        _ => AutoOutcome::Unknown,
    };
    if outcome == AutoOutcome::Error && events.is_empty() {
        let m = if !out.stderr.is_empty() {
            &out.stderr
        } else if !out.stdout.is_empty() {
            &out.stdout
        } else {
            "exit 1"
        };
        return Err(CswapError::new("CliError", m.trim()));
    }
    Ok(AutoOnceResult { exit_code: out.exit_code, outcome, events })
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '\x1b' && it.peek() == Some(&'[') {
            it.next();
            // `\x1b[` [0-9;]* `m`; anything else is left as it was.
            let mut seq = String::new();
            while let Some(&d) = it.peek() {
                if d.is_ascii_digit() || d == ';' {
                    seq.push(d);
                    it.next();
                } else {
                    break;
                }
            }
            if it.peek() == Some(&'m') {
                it.next();
            } else {
                out.push_str("\x1b[");
                out.push_str(&seq);
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MappingRow {
    pub path: String,
    pub email: String,
    pub number: Option<u32>,
}

/// `cswap map` has no --json; its lines look like
/// `<path> → 2: me@example.com [tag]` or `<path> → me@example.com (account removed)`.
pub fn parse_mappings(text: &str) -> Vec<MappingRow> {
    let mut rows = Vec::new();
    for raw in text.lines() {
        let line = strip_ansi(raw);
        let line = line.trim();
        let lower = line.to_lowercase();
        if line.is_empty()
            || lower.starts_with("no directory mappings")
            || lower.starts_with("map one with")
            || lower == "directory mappings"
            || lower == "directory mappings:"
        {
            continue;
        }
        if let Some(r) = mapping_line(line) {
            rows.push(r);
        }
    }
    rows
}

// `^(.*?)\s+(?:→|->)\s+(?:(\d+):\s+)?(\S+@\S+)`: the first arrow with whitespace on both
// sides splits path from account.
fn mapping_line(line: &str) -> Option<MappingRow> {
    for (i, _) in line.char_indices() {
        let rest = &line[i..];
        let arrow = if rest.starts_with('→') {
            '→'.len_utf8()
        } else if rest.starts_with("->") {
            2
        } else {
            continue;
        };
        let before = &line[..i];
        let after = &rest[arrow..];
        if before.is_empty()
            || !before.ends_with(char::is_whitespace)
            || !after.starts_with(char::is_whitespace)
            || before.trim().is_empty()
        {
            continue;
        }
        let after = after.trim_start();
        let (number, account) = match after.split_once(':') {
            Some((n, tail))
                if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && tail.starts_with(char::is_whitespace) =>
            {
                (n.parse().ok(), tail.trim_start())
            }
            _ => (None, after),
        };
        let email = account.split_whitespace().next().unwrap_or("");
        let Some(at) = email.find('@') else { continue };
        if at == 0 || at + 1 >= email.len() {
            continue;
        }
        return Some(MappingRow { path: before.trim().to_owned(), email: email.to_owned(), number });
    }
    None
}

/// `cswap unclaimed`: `<id>  slot <slot>  <reason>` rows.
pub fn parse_unclaimed(text: &str) -> Vec<UnclaimedEntry> {
    let mut rows = Vec::new();
    for raw in text.lines() {
        let line = strip_ansi(raw);
        let line = line.trim();
        if line.is_empty() || line.to_lowercase().starts_with("no unclaimed") {
            continue;
        }
        let mut parts = line.splitn(2, char::is_whitespace);
        let (Some(id), Some(rest)) = (parts.next(), parts.next()) else { continue };
        let rest = rest.trim_start();
        let Some(rest) = rest.strip_prefix("slot") else { continue };
        if !rest.starts_with(char::is_whitespace) {
            continue;
        }
        let mut parts = rest.trim_start().splitn(2, char::is_whitespace);
        let (Some(slot), Some(reason)) = (parts.next(), parts.next()) else { continue };
        rows.push(UnclaimedEntry { id: id.to_owned(), slot: slot.to_owned(), reason: reason.trim_start().to_owned() });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(stdout: &str, stderr: &str, code: Option<i32>) -> RawOutcome {
        RawOutcome { stdout: stdout.into(), stderr: stderr.into(), exit_code: code, ..Default::default() }
    }

    #[test]
    fn parse_mappings_reads_cswap_map_output_with_and_without_a_resolved_slot() {
        let text = "Directory mappings:\n  /home/me/work → 2: me@example.com [personal]\n  D:\\proj → gone@example.com (account removed)\n";
        assert_eq!(
            parse_mappings(text),
            vec![
                MappingRow { path: "/home/me/work".into(), email: "me@example.com".into(), number: Some(2) },
                MappingRow { path: "D:\\proj".into(), email: "gone@example.com".into(), number: None },
            ]
        );
        assert_eq!(parse_mappings("No directory mappings yet.\nMap one with: cswap map <NUM|EMAIL> [PATH]\n"), vec![]);
        assert_eq!(
            parse_mappings("\x1b[1m/a b/c\x1b[0m -> 3: x@example.com\r\n"),
            vec![MappingRow { path: "/a b/c".into(), email: "x@example.com".into(), number: Some(3) }]
        );
    }

    #[test]
    fn parse_unclaimed_reads_id_slot_reason_rows() {
        assert_eq!(
            parse_unclaimed("abc123  slot 2  keychain locked\n"),
            vec![UnclaimedEntry { id: "abc123".into(), slot: "2".into(), reason: "keychain locked".into() }]
        );
        assert_eq!(parse_unclaimed("No unclaimed credential entries"), vec![]);
    }

    #[test]
    fn json_errors_win_over_the_exit_code_and_noise_before_the_document_is_skipped() {
        let r: CsResult<ListPayload> =
            parse_json(&out("Overwrite? [y/N] {\"schemaVersion\":1,\"accounts\":[]}", "", Some(0)));
        assert_eq!(r.unwrap().schema_version, 1);
        let r: CsResult<ListPayload> =
            parse_json(&out("{\"schemaVersion\":1,\"error\":{\"type\":\"X\",\"message\":\"m\"}}", "", Some(0)));
        assert_eq!(r.unwrap_err(), CswapError::new("X", "m"));
        let r: CsResult<ListPayload> = parse_json(&out("garbage", "  boom \n", Some(1)));
        assert_eq!(r.unwrap_err(), CswapError::new("CliError", "boom"));
        let r: CsResult<ListPayload> = parse_json(&out("", "", Some(2)));
        assert_eq!(r.unwrap_err().message, "exit 2");
    }

    #[test]
    fn plain_errors_drop_the_error_prefix() {
        assert_eq!(plain(&out("", "Error: no such account\n", Some(1))).unwrap_err().message, "no such account");
        assert_eq!(plain(&out("ok", "", Some(0))).unwrap().stdout, "ok");
        let spawn = RawOutcome { spawn_error: Some("nope".into()), ..Default::default() };
        assert_eq!(plain(&spawn).unwrap_err().kind, "SpawnError");
    }

    #[test]
    fn auto_once_maps_exit_codes_and_keeps_events() {
        let o = out("noise\n{\"schemaVersion\":1,\"event\":\"poll\",\"ts\":\"t\"}\n", "", Some(2));
        let r = parse_auto_once(&o).unwrap();
        assert_eq!(r.outcome, AutoOutcome::NoAction);
        assert_eq!(r.events.len(), 1);
        assert_eq!(parse_auto_once(&out("", "bad", Some(1))).unwrap_err().message, "bad");
        assert_eq!(parse_auto_once(&out("", "", Some(3))).unwrap().outcome, AutoOutcome::Blocked);
        assert_eq!(parse_auto_once(&out("", "", None)).unwrap().outcome, AutoOutcome::Unknown);
    }
}
