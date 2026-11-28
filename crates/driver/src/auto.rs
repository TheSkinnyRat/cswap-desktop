//! The long-running `cswap auto --json` child (port of `src/main/cswap/auto.ts`). One at a
//! time; one JSON event per stdout line, kept in a ring buffer for the AUTO page.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use cswap_core::{AutoEvent, AutoStatus, Event, EventSink};
use serde_json::{Map, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::runtime::Handle;
use tokio::sync::{oneshot, watch};

use crate::proc;
use crate::time::now_iso;

/// Events kept for the AUTO page (0.1.23's size).
pub const EVENT_CAP: usize = 500;
/// After a stop request, how long a child gets before it is killed outright.
const KILL_AFTER: Duration = Duration::from_secs(5);

struct Running {
    generation: u64,
    pid: Option<u32>,
    kill: Option<oneshot::Sender<()>>,
}

struct Inner {
    status: AutoStatus,
    events: VecDeque<AutoEvent>,
    child: Option<Running>,
    generation: u64,
}

pub type OnEvent = Arc<dyn Fn(&AutoEvent) + Send + Sync>;

pub struct AutoRunner {
    rt: Handle,
    inner: Mutex<Inner>,
    /// Bumped each time a child is gone, so `stop` can wait for its own child's exit.
    exited: watch::Sender<u64>,
    sink: Option<EventSink>,
    on_event: Option<OnEvent>,
}

fn event(kind: &str, fields: &[(&str, Value)]) -> AutoEvent {
    let mut extra = Map::new();
    for (k, v) in fields {
        extra.insert((*k).to_owned(), v.clone());
    }
    AutoEvent { schema_version: 1, event: kind.to_owned(), ts: now_iso(), extra }
}

/// One stdout line as an event: cswap's error envelope becomes an `error` event, anything
/// that is not an event object is kept as `raw` (nothing the child says is dropped).
pub fn parse_line(line: &str) -> AutoEvent {
    let v = serde_json::from_str::<Value>(line).ok().filter(Value::is_object);
    let Some(v) = v else { return event("raw", &[("message", Value::String(line.to_owned()))]) };
    if let Some(e) = v.get("error").filter(|e| !e.is_null() && **e != Value::Bool(false)) {
        let msg = e.get("message").and_then(Value::as_str).map_or_else(|| e.to_string(), str::to_owned);
        return event("error", &[("message", Value::String(msg)), ("transient", Value::Bool(false))]);
    }
    match serde_json::from_value::<AutoEvent>(v) {
        Ok(mut ev) => {
            if ev.ts.is_empty() {
                ev.ts = now_iso();
            }
            ev
        }
        Err(_) => event("raw", &[("message", Value::String(line.to_owned()))]),
    }
}

impl AutoRunner {
    pub fn new(rt: Handle, sink: Option<EventSink>, on_event: Option<OnEvent>) -> Arc<Self> {
        Arc::new(Self {
            rt,
            inner: Mutex::new(Inner {
                status: AutoStatus::default(),
                events: VecDeque::new(),
                child: None,
                generation: 0,
            }),
            exited: watch::channel(0).0,
            sink,
            on_event,
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn status(&self) -> AutoStatus {
        self.lock().status.clone()
    }

    pub fn events(&self) -> Vec<AutoEvent> {
        self.lock().events.iter().cloned().collect()
    }

    fn emit_status(&self) {
        if let Some(s) = &self.sink {
            s.send(Event::AutoStatus(self.status()));
        }
    }

    fn push(&self, ev: AutoEvent) {
        {
            let mut g = self.lock();
            g.events.push_back(ev.clone());
            while g.events.len() > EVENT_CAP {
                g.events.pop_front();
            }
        }
        if let Some(s) = &self.sink {
            s.send(Event::AutoEvent(ev.clone()));
        }
        if let Some(f) = &self.on_event {
            f(&ev);
        }
    }

    /// Starts the child unless one is running (then its status is returned unchanged).
    pub fn start(self: &Arc<Self>, bin: Option<PathBuf>, env: Vec<(String, String)>, dry_run: bool) -> AutoStatus {
        let mut g = self.lock();
        if g.child.is_some() {
            return g.status.clone();
        }
        let Some(bin) = bin else {
            g.status = AutoStatus {
                running: false,
                last_error: Some("cswap binary not configured".into()),
                ..Default::default()
            };
            drop(g);
            self.emit_status();
            return self.status();
        };
        let mut args = vec!["auto".to_owned(), "--json".to_owned()];
        if dry_run {
            args.push("--dry-run".into());
        }
        let _enter = self.rt.enter();
        let mut cmd = proc::command(&bin, &args);
        for (k, v) in &env {
            cmd.env(k, v);
        }
        // Line-by-line events need the child to flush per line, not per 8 KB block.
        cmd.env("PYTHONUNBUFFERED", "1");
        // The child never reads stdin; null, never ours (see proc.rs).
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                g.status = AutoStatus { running: false, last_error: Some(e.to_string()), ..Default::default() };
                drop(g);
                self.emit_status();
                return self.status();
            }
        };
        g.generation += 1;
        let generation = g.generation;
        let pid = child.id();
        let (kill_tx, kill_rx) = oneshot::channel();
        g.child = Some(Running { generation, pid, kill: Some(kill_tx) });
        g.status = AutoStatus {
            running: true,
            pid,
            started_at: Some(now_iso()),
            dry_run: Some(dry_run),
            exit_code: None,
            last_error: None,
        };
        drop(g);

        let out = child.stdout.take().map(|r| {
            let me = self.clone();
            self.rt.spawn(async move { me.read_lines(r, false).await })
        });
        if let Some(r) = child.stderr.take() {
            let me = self.clone();
            self.rt.spawn(async move { me.read_lines(r, true).await });
        }
        let me = self.clone();
        self.rt.spawn(async move {
            let status = tokio::select! {
                s = child.wait() => s,
                _ = kill_rx => {
                    let _ = child.start_kill();
                    child.wait().await
                }
            };
            // Every line the child wrote is in before the status says it is gone.
            if let Some(t) = out {
                let _ = tokio::time::timeout(Duration::from_secs(2), t).await;
            }
            {
                let mut g = me.lock();
                if g.child.as_ref().is_some_and(|c| c.generation == generation) {
                    g.child = None;
                }
                g.status.running = false;
                g.status.pid = None;
                match status {
                    Ok(s) => g.status.exit_code = s.code(),
                    Err(e) => g.status.last_error = Some(e.to_string()),
                }
            }
            me.exited.send_modify(|n| *n += 1);
            me.emit_status();
        });
        self.emit_status();
        self.status()
    }

    async fn read_lines(&self, r: impl AsyncRead + Unpin, stderr: bool) {
        let mut r = BufReader::new(r);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match r.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => return,
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&buf);
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if stderr {
                self.push(event("stderr", &[("message", Value::String(line.to_owned()))]));
            } else {
                self.push(parse_line(line));
            }
        }
    }

    /// Asks the child to stop and waits until it has. POSIX: SIGTERM, which cswap handles
    /// cleanly; Windows has no signal, so the tree is killed. Either way a child still
    /// there after 5 s is killed outright.
    pub async fn stop(&self) -> AutoStatus {
        let (generation, pid) = {
            let g = self.lock();
            match &g.child {
                None => return g.status.clone(),
                Some(c) => (c.generation, c.pid),
            }
        };
        let mut rx = self.exited.subscribe();
        let gone = |me: &Self| me.lock().child.as_ref().is_none_or(|c| c.generation != generation);
        if let Some(pid) = pid {
            #[cfg(unix)]
            proc::signal_group(pid, libc::SIGTERM);
            #[cfg(windows)]
            proc::kill_tree(pid).await;
        }
        let wait = async {
            while !gone(self) {
                if rx.changed().await.is_err() {
                    return;
                }
            }
        };
        if tokio::time::timeout(KILL_AFTER, wait).await.is_err() {
            self.force_kill(generation, pid);
            let mut rx = self.exited.subscribe();
            let wait = async {
                while !gone(self) {
                    if rx.changed().await.is_err() {
                        return;
                    }
                }
            };
            let _ = tokio::time::timeout(KILL_AFTER, wait).await;
        }
        self.status()
    }

    fn force_kill(&self, generation: u64, pid: Option<u32>) {
        let kill = {
            let mut g = self.lock();
            g.child.as_mut().filter(|c| c.generation == generation).and_then(|c| c.kill.take())
        };
        #[cfg(unix)]
        if let Some(pid) = pid {
            proc::signal_group(pid, libc::SIGKILL);
        }
        #[cfg(not(unix))]
        let _ = pid;
        if let Some(k) = kill {
            let _ = k.send(());
        }
    }

    /// Blocking stop for shutdown: no runtime needed on the calling thread, bounded.
    pub fn stop_blocking(&self, limit: Duration) {
        let Some((generation, pid)) = self.lock().child.as_ref().map(|c| (c.generation, c.pid)) else { return };
        if let Some(pid) = pid {
            #[cfg(unix)]
            proc::signal_group(pid, libc::SIGTERM);
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = std::process::Command::new("taskkill")
                    .args(["/pid", &pid.to_string(), "/t", "/f"])
                    .creation_flags(proc::CREATE_NO_WINDOW)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
        let deadline = std::time::Instant::now() + limit;
        while std::time::Instant::now() < deadline {
            if self.lock().child.as_ref().is_none_or(|c| c.generation != generation) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        self.force_kill(generation, pid);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_become_events_and_nothing_is_dropped() {
        let ev = parse_line(r#"{"schemaVersion":1,"event":"poll","ts":"2026-01-01T00:00:00Z","threshold":90}"#);
        assert_eq!(ev.event, "poll");
        assert_eq!(ev.extra["threshold"], 90);
        let ev = parse_line(r#"{"schemaVersion":1,"event":"sleep"}"#);
        assert!(!ev.ts.is_empty(), "a missing ts is filled in");
        let ev = parse_line(r#"{"schemaVersion":1,"error":{"type":"ConfigError","message":"bad"}}"#);
        assert_eq!(
            (ev.event.as_str(), &ev.extra["message"], &ev.extra["transient"]),
            ("error", &Value::from("bad"), &Value::Bool(false))
        );
        let ev = parse_line("Traceback (most recent call last):");
        assert_eq!(
            (ev.event.as_str(), &ev.extra["message"]),
            ("raw", &Value::from("Traceback (most recent call last):"))
        );
    }
}
