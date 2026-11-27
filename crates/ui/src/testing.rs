//! Fakes for tests, the demo and the README screenshots: a [`RecordingHost`] that answers
//! dialogs from a queue and records what the window asked of the platform, and
//! [`DisabledUpdates`], an updater that is off (dev builds) but still records calls.
//! Public behind the `testing` feature; always compiled into this crate's tests.
//!
//! The `Cswap` side is `cswap_core::mock::MockCswap`.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard};

use cswap_core::*;

/// Everything the window asked of the platform, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostCall {
    PickDirectory(String),
    PickFile(String),
    SaveFile(String, String),
    OpenPath(String),
    OpenExternal(String),
    CloseWindow,
    Quit,
}

#[derive(Default)]
struct HostState {
    calls: Vec<HostCall>,
    answers: VecDeque<Option<String>>,
}

/// A [`Host`] that records calls and answers dialogs from [`RecordingHost::answer`] (a
/// dialog with no queued answer is "cancelled").
pub struct RecordingHost {
    s: Mutex<HostState>,
    pub version: String,
    pub platform: &'static str,
}

impl Default for RecordingHost {
    fn default() -> Self {
        RecordingHost { s: Mutex::default(), version: "1.0.0".into(), platform: "linux" }
    }
}

impl RecordingHost {
    pub fn new(platform: &'static str) -> Arc<Self> {
        Arc::new(RecordingHost { platform, ..Default::default() })
    }
    fn s(&self) -> MutexGuard<'_, HostState> {
        self.s.lock().unwrap_or_else(|e| e.into_inner())
    }
    /// Queues the answer of the next dialog (`None` = cancelled).
    pub fn answer(&self, a: Option<&str>) {
        self.s().answers.push_back(a.map(Into::into));
    }
    pub fn calls(&self) -> Vec<HostCall> {
        self.s().calls.clone()
    }
    fn dialog(&self, call: HostCall) -> Option<String> {
        let mut s = self.s();
        s.calls.push(call);
        s.answers.pop_front().flatten()
    }
}

impl Host for RecordingHost {
    fn pick_directory(&self, title: &str) -> Option<String> {
        self.dialog(HostCall::PickDirectory(title.into()))
    }
    fn pick_file(&self, title: &str, _filters: &[(&str, &[&str])]) -> Option<String> {
        self.dialog(HostCall::PickFile(title.into()))
    }
    fn save_file(&self, title: &str, default_name: &str) -> Option<String> {
        self.dialog(HostCall::SaveFile(title.into(), default_name.into()))
    }
    fn open_path(&self, path: &str) {
        self.s().calls.push(HostCall::OpenPath(path.into()));
    }
    fn open_external(&self, url: &str) {
        self.s().calls.push(HostCall::OpenExternal(url.into()));
    }
    fn app_version(&self) -> String {
        self.version.clone()
    }
    fn platform(&self) -> &'static str {
        self.platform
    }
    fn close_window(&self) {
        self.s().calls.push(HostCall::CloseWindow);
    }
    fn quit(&self) {
        self.s().calls.push(HostCall::Quit);
    }
}

/// An [`Updates`] whose state is whatever the test sets; `check` answers with it at once.
pub struct DisabledUpdates {
    state: Mutex<UpdaterState>,
    pub checks: Mutex<u32>,
    pub installs: Mutex<u32>,
}

impl DisabledUpdates {
    pub fn new(current: &str) -> Arc<Self> {
        Arc::new(DisabledUpdates {
            state: Mutex::new(UpdaterState {
                status: UpdaterStatus::Disabled,
                current: current.into(),
                release_url: Some("https://github.com/TheSkinnyRat/cswap-desktop/releases/latest".into()),
                disabled_reason: Some("Running from source or a dev build — use the release page.".into()),
                ..Default::default()
            }),
            checks: Mutex::new(0),
            installs: Mutex::new(0),
        })
    }
    pub fn set(&self, s: UpdaterState) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }
}

impl Updates for DisabledUpdates {
    fn state(&self) -> UpdaterState {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn check(&self) -> Pending<UpdaterState> {
        *self.checks.lock().unwrap_or_else(|e| e.into_inner()) += 1;
        Pending::ready(self.state())
    }
    fn install(&self) {
        *self.installs.lock().unwrap_or_else(|e| e.into_inner()) += 1;
    }
}
