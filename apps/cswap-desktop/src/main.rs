//! Thin app: single instance, tray, the window's lifetime, the platform [`cswap_core::Host`],
//! launch at login, notifications, and wiring driver + updater + UI together.
//! OWNER: platform.

// No console window in a release build: this is a tray app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod host;
mod instance;
mod notify;
mod shell;
#[cfg(feature = "stub-driver")]
mod stub;
mod tray;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use cswap_core::{AppSettings, Cswap, Event, EventSink};
use cswap_ui::Services;
use cswap_update::{UpdateConfig, Updater};

use crate::shell::Control;

const REPO: &str = "TheSkinnyRat/cswap-desktop";

#[cfg(not(feature = "stub-driver"))]
fn data_dir() -> PathBuf {
    cswap_driver::data_dir()
}

#[cfg(feature = "stub-driver")]
fn data_dir() -> PathBuf {
    stub::data_dir()
}

#[cfg(not(feature = "stub-driver"))]
fn start_driver(data_dir: PathBuf, sink: EventSink) -> (Arc<dyn Cswap>, Box<dyn FnOnce()>) {
    let native = cswap_driver::Native::start(cswap_driver::DriverConfig { data_dir }, sink);
    let n = native.clone();
    (native, Box::new(move || n.shutdown()))
}

#[cfg(feature = "stub-driver")]
fn start_driver(data_dir: PathBuf, sink: EventSink) -> (Arc<dyn Cswap>, Box<dyn FnOnce()>) {
    (stub::StubCswap::start(data_dir, sink), Box::new(|| {}))
}

/// A copy the app may let rewire the user's login items: not a cargo build, not a test run
/// with its own data dir (0.1.23 checked `app.isPackaged`).
fn packaged() -> bool {
    std::env::var_os("CSWAP_DESKTOP_USERDATA").is_none()
        && cswap_update::kind::Probe::current() != cswap_update::kind::InstallKind::Dev
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().collect();
    // The update helper, or 0.1.x's electron-updater test-running a new AppImage.
    if let Some(done) = cswap_update::finish(&args) {
        return match done {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("claude-swap: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("claude-swap {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    let hidden = args.iter().any(|a| a == autostart::HIDDEN_FLAG);
    let data = data_dir();
    let instance = match instance::claim(&data) {
        Ok(instance::Claim::Primary(p)) => Some(p),
        Ok(instance::Claim::Secondary) => return ExitCode::SUCCESS,
        Err(e) => {
            // Better two copies than none.
            eprintln!("claude-swap: single-instance check failed: {e}");
            None
        }
    };

    // One runtime for the process; the driver and the updater spawn on it from `start`.
    let rt = match tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("cswap-rt").build() {
        Ok(rt) => rt,
        Err(e) => {
            eprintln!("claude-swap: no async runtime: {e}");
            return ExitCode::FAILURE;
        }
    };
    let _entered = rt.enter();

    let (sink, events) = EventSink::new();
    let (control, woken) = Control::new();
    let (cswap, shutdown_driver) = start_driver(data.clone(), sink.clone());
    let settings = cswap.settings();
    let quit = {
        let c = control.clone();
        Arc::new(move || c.request_quit()) as Arc<dyn Fn() + Send + Sync>
    };
    let updater = Updater::start(
        UpdateConfig {
            repo: REPO.into(),
            current: env!("CARGO_PKG_VERSION").into(),
            data_dir: data.clone(),
            auto: settings.auto_update,
        },
        sink.clone(),
        quit,
    );
    if let Some(p) = &instance {
        let c = control.clone();
        p.on_show(move || c.request_show());
    }
    *control.model.lock().unwrap_or_else(|p| p.into_inner()) =
        tray::Model::from_state(&cswap.accounts(), &cswap.auto_status());
    spawn_router(events, control.clone(), cswap.clone(), updater.clone(), settings.clone());

    let services =
        Services { cswap, updates: updater.clone(), host: Arc::new(host::DesktopHost { control: control.clone() }) };
    let core = shell::Core::new(services, sink, control, woken);
    let show = !(hidden || settings.start_minimized);
    let result = shell::run(core, show);

    // 0.1.x's autoInstallOnAppQuit: a downloaded update goes in as the app quits.
    updater.install_on_quit();
    shutdown_driver();
    drop(instance);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("claude-swap: the window failed: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Every pushed event passes here: the tray's model, auto-switch notifications, settings
/// that the app itself acts on, then on to the window if one is open.
fn spawn_router(
    events: Receiver<Event>,
    control: Arc<Control>,
    cswap: Arc<dyn Cswap>,
    updater: Arc<Updater>,
    mut last: AppSettings,
) {
    let packaged = packaged();
    std::thread::Builder::new()
        .name("event-router".into())
        .spawn(move || {
            for ev in events {
                match &ev {
                    Event::Accounts(state) => {
                        control.model.lock().unwrap_or_else(|p| p.into_inner()).accounts =
                            state.payload.as_ref().map(|p| p.accounts.clone()).unwrap_or_default();
                        control.tray_changed();
                    }
                    Event::AutoStatus(st) => {
                        control.model.lock().unwrap_or_else(|p| p.into_inner()).auto = st.clone();
                        control.tray_changed();
                    }
                    Event::AutoEvent(e) => {
                        if cswap.settings().notify_on_auto_switch
                            && let Some((title, body)) = notify::text(e)
                        {
                            notify::show(title, body);
                        }
                    }
                    Event::Settings(s) => {
                        // As 0.1.23's setSettings: the login item follows a CHANGE of the
                        // setting, in a packaged copy only.
                        if s.launch_at_login != last.launch_at_login
                            && packaged
                            && autostart::is_enabled() != s.launch_at_login
                        {
                            if let Err(e) = autostart::set_enabled(s.launch_at_login) {
                                tracing::warn!("launch at login: {e}");
                            }
                        }
                        if s.auto_update != last.auto_update {
                            updater.set_auto(s.auto_update);
                        }
                        last = s.clone();
                    }
                    Event::Navigate(_) => control.request_show(),
                    _ => {}
                }
                control.forward(ev);
            }
        })
        .expect("spawn the event router");
}
