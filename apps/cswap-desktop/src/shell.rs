//! The process's UI lifetime: the tray and the event router run all the time; the window
//! exists only while it is open (Windows, Linux).
//!
//! An eframe window brings the OpenGL driver with it, and the driver stays loaded once it
//! has been (Reina Agent measured ~105 MB private with its window never shown, against
//! ~4 MB without). So there is no window until one is asked for (tray, a second launch, a
//! start that is not `--hidden`/start-minimized). Closing it to the tray ends that eframe
//! pass (`run_and_return`) and a plain loop on this main thread keeps the tray alive; the
//! next show builds a fresh window, which re-reads state from the services' getters.
//! Structure after craft-launcher's `wait_in_tray` and Reina Agent 1.0.1's shell.
//!
//! macOS is different: the tray (NSStatusItem) only works while AppKit's run loop runs,
//! which is winit's, and winit cannot run a second event loop after the first returned.
//! So there the one window lives until Quit, and closing to the menu bar HIDES THE APP
//! (as Cmd+H; craft-launcher's choice): the Dock, Finder and Cmd+Tab bring a hidden app
//! back by themselves, while an ordered-out window would stay gone (winit does not pass
//! the Dock's "reopen" on). Memory is not given back there.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use cswap_core::{Event, RotateStrategy};
use cswap_ui::{CswapApp, Services};
use eframe::egui::{self, ViewportCommand};

use crate::tray::{self, Action, Tray};

/// How often the loop looks at the world while nothing wakes it.
const POLL: Duration = Duration::from_millis(250);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Requests from other threads (tray, second launch, router, Host, updater) to the main
/// thread, and what they need to reach the open window.
pub struct Control {
    show: AtomicBool,
    close: AtomicBool,
    quit: AtomicBool,
    tray_dirty: AtomicBool,
    wake_tx: SyncSender<()>,
    window: Mutex<Option<(egui::Context, Sender<Event>)>>,
    /// A page asked for while no window was open, delivered to the next one.
    navigate: Mutex<Option<Event>>,
    pub model: Mutex<tray::Model>,
}

impl Control {
    pub fn new() -> (Arc<Control>, Receiver<()>) {
        let (wake_tx, woken) = sync_channel(1);
        let c = Control {
            show: AtomicBool::new(false),
            close: AtomicBool::new(false),
            quit: AtomicBool::new(false),
            tray_dirty: AtomicBool::new(true),
            wake_tx,
            window: Mutex::new(None),
            navigate: Mutex::new(None),
            model: Mutex::new(tray::Model::default()),
        };
        (Arc::new(c), woken)
    }

    /// Wakes the window if there is one, and the tray loop otherwise.
    pub fn wake(&self) {
        if let Some((ctx, _)) = lock(&self.window).as_ref() {
            ctx.request_repaint();
        }
        let _ = self.wake_tx.try_send(());
    }

    pub fn request_show(&self) {
        self.show.store(true, Ordering::SeqCst);
        self.wake();
    }

    pub fn request_close(&self) {
        self.close.store(true, Ordering::SeqCst);
        self.wake();
    }

    pub fn request_quit(&self) {
        self.quit.store(true, Ordering::SeqCst);
        self.wake();
    }

    pub fn quitting(&self) -> bool {
        self.quit.load(Ordering::SeqCst)
    }

    pub fn tray_changed(&self) {
        self.tray_dirty.store(true, Ordering::SeqCst);
        self.wake();
    }

    /// An event for the open window, if any.
    pub fn forward(&self, ev: Event) {
        match lock(&self.window).as_ref() {
            Some((ctx, tx)) => {
                let _ = tx.send(ev);
                ctx.request_repaint();
            }
            None if matches!(ev, Event::Navigate(_)) => *lock(&self.navigate) = Some(ev),
            None => {}
        }
    }

    fn set_window(&self, w: Option<(egui::Context, Sender<Event>)>) {
        if let Some((_, tx)) = &w
            && let Some(ev) = lock(&self.navigate).take()
        {
            let _ = tx.send(ev);
        }
        *lock(&self.window) = w;
    }
}

/// What the loop does next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Stay,
    Show,
    Hide,
    Exit,
}

/// What a close (the window's X, or the UI's own close) does, as 0.1.23: hide to the tray
/// when close-to-tray is on AND there is a tray to come back from; quit otherwise.
pub fn closes_to_tray(close_to_tray: bool, has_tray: bool) -> bool {
    close_to_tray && has_tray
}

/// Everything that outlives a window.
pub struct Core {
    services: Services,
    sink: cswap_core::EventSink,
    control: Arc<Control>,
    woken: Receiver<()>,
    tray: Option<Tray>,
    /// The window is open and not hidden (for the tray icon's toggle click).
    visible: bool,
    exiting: bool,
}

impl Core {
    pub fn new(services: Services, sink: cswap_core::EventSink, control: Arc<Control>, woken: Receiver<()>) -> Core {
        let wake = {
            let c = control.clone();
            move || c.wake()
        };
        let tray = match Tray::new(wake) {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::warn!("tray icon unavailable: {e}");
                None
            }
        };
        Core { services, sink, control, woken, tray, visible: false, exiting: false }
    }

    pub fn has_tray(&self) -> bool {
        self.tray.is_some()
    }

    /// One look at the world, window or not.
    fn tick(&mut self) -> Step {
        let mut step = Step::Stay;
        for a in self.tray.as_ref().map(|t| t.actions()).unwrap_or_default() {
            match a {
                Action::Open => step = Step::Show,
                Action::Toggle if self.visible && !cfg!(target_os = "macos") => step = Step::Hide,
                Action::Toggle => step = Step::Show,
                Action::Quit => self.control.quit.store(true, Ordering::SeqCst),
                other => self.run_action(other),
            }
        }
        if self.control.show.swap(false, Ordering::SeqCst) {
            step = Step::Show;
        }
        if self.control.close.swap(false, Ordering::SeqCst) {
            let s = self.services.cswap.settings();
            step = if closes_to_tray(s.close_to_tray, self.has_tray()) {
                Step::Hide
            } else {
                self.control.quit.store(true, Ordering::SeqCst);
                Step::Exit
            };
        }
        if self.control.quitting() {
            return Step::Exit;
        }
        if self.control.tray_dirty.swap(false, Ordering::SeqCst)
            && let Some(t) = self.tray.as_mut()
        {
            let m = lock(&self.control.model).clone();
            t.sync(&m);
        }
        step
    }

    /// Tray menu items that go to cswap. Their replies come back as `Event::Accounts`
    /// (the driver refreshes after every mutation), so the Pending is not kept.
    fn run_action(&self, a: Action) {
        let c = &self.services.cswap;
        match a {
            Action::Switch(n) => drop(c.switch_to(&n.to_string(), false)),
            Action::RotateNext => drop(c.rotate(None)),
            Action::SwitchBest => drop(c.rotate(Some(RotateStrategy::Best))),
            Action::Refresh => drop(c.refresh()),
            Action::ToggleAuto => {
                if c.auto_status().running {
                    drop(c.auto_stop());
                } else {
                    drop(c.auto_start(c.settings().auto_dry_run));
                }
            }
            Action::Open | Action::Toggle | Action::Quit => {}
        }
    }

    /// No window: carry on until it is asked for (`Show`) or the app exits.
    #[cfg(not(target_os = "macos"))]
    fn wait_in_tray(&mut self) -> Step {
        self.visible = false;
        loop {
            match self.tick() {
                Step::Show => return Step::Show,
                Step::Exit => return Step::Exit,
                Step::Stay | Step::Hide => {}
            }
            self.wait(POLL);
        }
    }

    /// Sleeps until woken or `timeout`, dispatching the tray's window messages on Windows
    /// (its hidden window lives on this thread).
    #[cfg(windows)]
    #[allow(dead_code)]
    fn wait(&self, timeout: Duration) {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE, PeekMessageW, QS_ALLINPUT, TranslateMessage,
        };
        // A channel cannot be waited on together with messages; the tray's clicks arrive as
        // messages anyway, and other wakes are picked up within 100 ms.
        let timeout = timeout.min(Duration::from_millis(100));
        // SAFETY: standard message pumping on this thread with a live MSG.
        unsafe {
            MsgWaitForMultipleObjectsEx(0, std::ptr::null(), timeout.as_millis() as u32, QS_ALLINPUT, MWMO_INPUTAVAILABLE);
            let mut msg: MSG = std::mem::zeroed();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        while self.woken.try_recv().is_ok() {}
    }

    #[cfg(not(windows))]
    #[allow(dead_code)]
    fn wait(&self, timeout: Duration) {
        let _ = self.woken.recv_timeout(timeout);
    }

    /// Takes the tray icon down so it does not linger while the process ends.
    fn shut_down(&mut self) {
        self.tray = None;
    }
}

/// Runs the UI on the main thread until Quit. `show` = open the window at start.
pub fn run(mut core: Core, mut show: bool) -> Result<(), String> {
    // Started hidden but nothing to come back from: the window opens after all.
    if !core.has_tray() {
        show = true;
    }
    let result = run_platform(&mut core, show);
    core.shut_down();
    result
}

#[cfg(not(target_os = "macos"))]
fn run_platform(core: &mut Core, mut show: bool) -> Result<(), String> {
    loop {
        if show {
            open_window(core, true)?;
            if core.exiting {
                return Ok(());
            }
        }
        match core.wait_in_tray() {
            Step::Exit => return Ok(()),
            _ => show = true,
        }
    }
}

#[cfg(target_os = "macos")]
fn run_platform(core: &mut Core, show: bool) -> Result<(), String> {
    // One pass for the whole life of the process (see the module comment).
    open_window(core, show)
}

fn native_options(visible: bool) -> eframe::NativeOptions {
    // 0.1.23's BrowserWindow: 1120x720, min 760x520.
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Claude Swap")
        .with_app_id("claude-swap")
        .with_inner_size([1120.0, 720.0])
        .with_min_inner_size([760.0, 520.0])
        .with_visible(visible);
    // macOS: the bundle's .icns is the icon; any icon given here replaces it in the Dock
    // with a raw bitmap (craft-launcher), so give the empty one eframe leaves alone.
    if cfg!(target_os = "macos") {
        viewport = viewport.with_icon(egui::IconData::default());
    } else if let Some((rgba, width, height)) = tray::decode_png(tray::APP_ICON) {
        viewport = viewport.with_icon(Arc::new(egui::IconData { rgba, width, height }));
    }
    eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        // Closing must come back here, not end the process: eframe's other mode calls
        // `process::exit` from inside the event loop.
        run_and_return: true,
        centered: true,
        ..Default::default()
    }
}

/// One eframe pass: from opening the window until it closes to the tray or the app exits.
fn open_window(core: &mut Core, visible: bool) -> Result<(), String> {
    let c = &mut *core;
    let r = eframe::run_native("Claude Swap", native_options(visible), Box::new(move |cc| Ok(Box::new(Window::new(cc, c, visible)))));
    // Whether or not eframe called `on_exit`: nothing may reach a window that is gone.
    core.control.set_window(None);
    core.sink.set_waker(None);
    core.visible = false;
    r.map_err(|e| e.to_string())
}

/// The window, for one eframe pass.
struct Window<'a> {
    core: &'a mut Core,
    app: CswapApp,
    /// Frames left in which to keep pulling the window to the front.
    raise_frames: u8,
    /// A close is under way to the tray: let it through.
    closing: bool,
}

impl<'a> Window<'a> {
    fn new(cc: &eframe::CreationContext<'_>, core: &'a mut Core, visible: bool) -> Self {
        let (tx, rx) = channel();
        let ctx = cc.egui_ctx.clone();
        core.control.set_window(Some((ctx.clone(), tx)));
        let repaint = ctx.clone();
        core.sink.set_waker(Some(Arc::new(move || repaint.request_repaint())));
        let app = CswapApp::new(cc, core.services.clone(), rx);
        core.visible = visible;
        let mut w = Window { core, app, raise_frames: 0, closing: false };
        if visible {
            w.raise(&ctx);
        } else {
            #[cfg(target_os = "macos")]
            mac::hide();
        }
        w
    }

    fn raise(&mut self, ctx: &egui::Context) {
        #[cfg(target_os = "macos")]
        mac::unhide();
        self.core.visible = true;
        ctx.send_viewport_cmd(ViewportCommand::Visible(true));
        ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
        ctx.send_viewport_cmd(ViewportCommand::Focus);
        // `Focus` is refused while another app is in front (Windows' foreground lock),
        // so the window can come up behind it; keep asking for a few frames.
        self.raise_frames = 3;
        ctx.request_repaint();
    }

    /// Close to the tray: end this pass (Windows/Linux) or hide the app (macOS).
    fn put_away(&mut self, ctx: &egui::Context) {
        self.core.visible = false;
        #[cfg(target_os = "macos")]
        {
            let _ = ctx;
            mac::hide();
        }
        #[cfg(not(target_os = "macos"))]
        {
            self.closing = true;
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
    }

    fn exit(&mut self, ctx: &egui::Context) {
        self.core.exiting = true;
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }
}

impl eframe::App for Window<'_> {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.app.logic(ctx, frame);
        if self.core.exiting {
            return;
        }
        match self.core.tick() {
            Step::Stay => {}
            Step::Show => self.raise(ctx),
            Step::Hide => self.put_away(ctx),
            Step::Exit => return self.exit(ctx),
        }
        // The window's own X (or Alt+F4, Cmd+W).
        if ctx.input(|i| i.viewport().close_requested()) && !self.closing && !self.core.exiting {
            let s = self.core.services.cswap.settings();
            if closes_to_tray(s.close_to_tray, self.core.has_tray()) {
                #[cfg(target_os = "macos")]
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                self.put_away(ctx);
            } else {
                self.core.control.quit.store(true, Ordering::SeqCst);
                self.core.exiting = true;
            }
        }
        if self.raise_frames > 0 {
            self.raise_frames -= 1;
            if self.raise_frames == 0 {
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
            ctx.request_repaint();
        }
        ctx.request_repaint_after(POLL);
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.app.ui(ui, frame);
    }

    fn on_exit(&mut self) {
        self.core.control.set_window(None);
        self.core.sink.set_waker(None);
        self.app.on_exit();
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    /// Hides the app and its window (Cmd+H). Main thread only; elsewhere a no-op.
    pub fn hide() {
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).hide(None);
        }
    }

    pub fn unhide() {
        if let Some(mtm) = MainThreadMarker::new() {
            NSApplication::sharedApplication(mtm).unhide(None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn close_goes_to_the_tray_only_when_there_is_one() {
        assert!(closes_to_tray(true, true));
        assert!(!closes_to_tray(true, false), "no tray: closing must quit, or the app is unreachable");
        assert!(!closes_to_tray(false, true));
    }
}
