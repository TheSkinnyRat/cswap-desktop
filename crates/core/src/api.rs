//! The service contracts between the parts.
//!
//! The UI is immediate-mode and never blocks a frame: every call returns a [`Pending`] that
//! the UI keeps in its state and polls each frame; the service fills it from its own
//! threads and wakes the UI through the [`Waker`]. Pushed state (list refreshes, auto
//! events, the command log, ...) arrives as [`Event`]s through an [`EventSink`] that the app
//! creates once and routes to the tray and to the window, if one is open.

use std::sync::mpsc::{Receiver, Sender, SyncSender, TryRecvError, channel, sync_channel};
use std::sync::{Arc, Mutex};

use crate::settings::{AppSettings, SettingsEdit};
use crate::types::*;

/// Asks the UI for a repaint. Replaced when a window opens or closes; a no-op without one.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// A reply that has not necessarily arrived yet.
pub struct Pending<T> {
    rx: Receiver<T>,
    value: Option<T>,
    lost: bool,
}

/// The sending half of a [`Pending`]. Dropping it unfilled marks the reply lost, so a
/// service thread that dies never leaves a spinner turning forever.
pub struct Fill<T> {
    tx: SyncSender<T>,
    waker: Option<Waker>,
}

impl<T> Pending<T> {
    pub fn channel(waker: Option<Waker>) -> (Fill<T>, Pending<T>) {
        let (tx, rx) = sync_channel(1);
        (Fill { tx, waker }, Pending { rx, value: None, lost: false })
    }

    /// Already answered (mock services, cached state).
    pub fn ready(value: T) -> Self {
        let (_tx, rx) = sync_channel(1);
        Pending { rx, value: Some(value), lost: false }
    }

    /// Takes the reply once it is there. `None` while waiting, and forever after taking.
    pub fn take(&mut self) -> Option<T> {
        self.poll();
        self.value.take()
    }

    /// Looks at the reply without taking it.
    pub fn peek(&mut self) -> Option<&T> {
        self.poll();
        self.value.as_ref()
    }

    /// The filling side went away without answering.
    pub fn is_lost(&mut self) -> bool {
        self.poll();
        self.lost
    }

    fn poll(&mut self) {
        if self.value.is_some() || self.lost {
            return;
        }
        match self.rx.try_recv() {
            Ok(v) => self.value = Some(v),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => self.lost = true,
        }
    }
}

impl<T> Fill<T> {
    pub fn fill(self, value: T) {
        let _ = self.tx.send(value);
        if let Some(w) = &self.waker {
            w();
        }
    }
}

/// Pushed state. The app sees every event (tray menu, notifications); the window sees
/// them while it is open and re-reads current state through the getters when it opens.
#[derive(Debug, Clone)]
pub enum Event {
    Accounts(AccountsState),
    AutoEvent(AutoEvent),
    AutoStatus(AutoStatus),
    CommandLog(CommandLogEntry),
    Settings(AppSettings),
    Binary(CswapBinaryInfo),
    Updater(UpdaterState),
    /// The tray or a second launch asked for a page.
    Navigate(Page),
}

/// Where services push [`Event`]s. Cheap to clone; sending never blocks and never fails
/// visibly (no receiver = nobody to tell).
#[derive(Clone)]
pub struct EventSink {
    tx: Sender<Event>,
    waker: Arc<Mutex<Option<Waker>>>,
}

impl EventSink {
    pub fn new() -> (EventSink, Receiver<Event>) {
        let (tx, rx) = channel();
        (EventSink { tx, waker: Arc::new(Mutex::new(None)) }, rx)
    }

    pub fn send(&self, ev: Event) {
        let _ = self.tx.send(ev);
        if let Some(w) = self.waker() {
            w();
        }
    }

    /// The current waker, for services building a [`Pending`].
    pub fn waker(&self) -> Option<Waker> {
        self.waker.lock().ok().and_then(|g| g.clone())
    }

    pub fn set_waker(&self, waker: Option<Waker>) {
        if let Ok(mut g) = self.waker.lock() {
            *g = waker;
        }
    }
}

/// Everything that goes through the cswap CLI or the app's own state. Implemented by
/// `cswap-driver` (the real CLI) and [`crate::mock::MockCswap`] (in memory).
///
/// Mutations are serialized by the implementation and followed by a list refresh, which
/// arrives as [`Event::Accounts`]; callers do not refresh by hand.
pub trait Cswap: Send + Sync {
    // state
    fn accounts(&self) -> AccountsState;
    fn refresh(&self) -> Pending<AccountsState>;
    fn status(&self) -> Pending<CsResult<StatusPayload>>;
    // mutations
    fn switch_to(&self, target: &str, force: bool) -> Pending<CsResult<SwitchPayload>>;
    fn rotate(&self, strategy: Option<RotateStrategy>) -> Pending<CsResult<SwitchPayload>>;
    fn add_account(&self, opts: AddAccountOptions) -> Pending<CsResult<PlainOutput>>;
    fn add_token(&self, opts: AddTokenOptions) -> Pending<CsResult<PlainOutput>>;
    fn remove_account(&self, target: &str) -> Pending<CsResult<PlainOutput>>;
    fn set_disabled(&self, target: &str, disabled: bool) -> Pending<CsResult<PlainOutput>>;
    fn set_alias(&self, target: &str, alias: Option<&str>) -> Pending<CsResult<PlainOutput>>;
    fn move_account(&self, target: &str, slot: u32) -> Pending<CsResult<PlainOutput>>;
    fn swap_accounts(&self, a: &str, b: &str) -> Pending<CsResult<PlainOutput>>;
    // mappings
    fn list_mappings(&self) -> Pending<CsResult<Vec<Mapping>>>;
    fn map_directory(&self, target: &str, path: &str) -> Pending<CsResult<PlainOutput>>;
    fn unmap_directory(&self, path: &str) -> Pending<CsResult<PlainOutput>>;
    // cswap's own settings.json
    fn config(&self) -> Pending<CsResult<ConfigPayload>>;
    fn set_config(&self, key: &str, value: Option<&str>) -> Pending<CsResult<PlainOutput>>;
    // transfer; the path comes from the UI's file dialog
    fn export_accounts(&self, opts: ExportOptions, path: &str) -> Pending<CsResult<Transfer>>;
    fn import_accounts(&self, opts: ImportOptions, path: &str) -> Pending<CsResult<Transfer>>;
    // maintenance
    fn list_unclaimed(&self) -> Pending<CsResult<Vec<UnclaimedEntry>>>;
    fn purge_unclaimed(&self, id: &str) -> Pending<CsResult<PlainOutput>>;
    fn upgrade_cswap(&self) -> Pending<CsResult<PlainOutput>>;
    fn token_status(&self) -> Pending<CsResult<PlainOutput>>;
    fn launch_session(&self, target: &str, cwd: Option<&str>) -> Pending<CsResult<LaunchedSession>>;
    // auto-switch process
    fn auto_start(&self, dry_run: bool) -> Pending<AutoStatus>;
    fn auto_stop(&self) -> Pending<AutoStatus>;
    fn auto_status(&self) -> AutoStatus;
    fn auto_events(&self) -> Vec<AutoEvent>;
    fn auto_once(&self, dry_run: bool) -> Pending<CsResult<AutoOnceResult>>;
    // app settings and binary
    fn settings(&self) -> AppSettings;
    fn edit_settings(&self, edit: SettingsEdit) -> AppSettings;
    /// `None` until the first detection has finished.
    fn binary(&self) -> Option<CswapBinaryInfo>;
    fn detect_binary(&self) -> Pending<CswapBinaryInfo>;
    fn install_cswap(&self) -> Pending<CsResult<PlainOutput>>;
    fn command_log(&self) -> Vec<CommandLogEntry>;
    fn clear_command_log(&self);
    fn vault_path(&self) -> Option<String>;
}

/// Self-update. Implemented by `cswap-update`; a disabled stub in tests and dev builds.
pub trait Updates: Send + Sync {
    fn state(&self) -> UpdaterState;
    fn check(&self) -> Pending<UpdaterState>;
    /// Downloads if needed, verifies, then hands over to the installer and quits the app.
    fn install(&self);
}

/// What the window asks of the platform it runs on. Implemented by the app; a recording
/// fake in UI tests. Dialogs return `None` when cancelled.
pub trait Host: Send + Sync {
    fn pick_directory(&self, title: &str) -> Option<String>;
    fn pick_file(&self, title: &str, filters: &[(&str, &[&str])]) -> Option<String>;
    fn save_file(&self, title: &str, default_name: &str) -> Option<String>;
    fn open_path(&self, path: &str);
    fn open_external(&self, url: &str);
    fn app_version(&self) -> String;
    /// `windows` / `macos` / `linux`, for platform-specific copy in the UI.
    fn platform(&self) -> &'static str;
    /// Hide to tray (or quit, when close-to-tray is off). The app decides.
    fn close_window(&self);
    fn quit(&self);
}
