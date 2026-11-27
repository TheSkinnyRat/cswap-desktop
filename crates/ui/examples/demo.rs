//! The window against in-memory services: `cargo run -p cswap-ui --example demo`.
//! Nothing here touches cswap, the vault or the network; dialogs answer "cancelled".
//! `--dark` starts in the dark theme, `--no-cswap` shows the onboarding screen.

use std::sync::Arc;

use cswap_core::{CswapBinaryInfo, EventSink, ThemeSetting};
use cswap_ui::testing::{DisabledUpdates, RecordingHost, StubCswap};
use cswap_ui::{CswapApp, Services};

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    let (sink, events) = EventSink::new();
    let cswap = StubCswap::new(Some(sink.clone()));
    if args.iter().any(|a| a == "--dark") {
        cswap.st().settings.theme = ThemeSetting::Dark;
    }
    if args.iter().any(|a| a == "--no-cswap") {
        let none = CswapBinaryInfo { candidates: vec!["~/.local/bin".into()], ..Default::default() };
        cswap.st().binary = Some(none.clone());
        cswap.st().detect = Some(none);
    }
    let platform = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    };
    let services = Services { cswap, updates: DisabledUpdates::new("1.0.0"), host: RecordingHost::new(platform) };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Claude Swap (demo)")
            .with_inner_size([1240.0, 780.0])
            .with_min_inner_size([520.0, 420.0]),
        ..Default::default()
    };
    eframe::run_native(
        "claude-swap-demo",
        options,
        Box::new(move |cc| {
            let ctx = cc.egui_ctx.clone();
            sink.set_waker(Some(Arc::new(move || ctx.request_repaint())));
            Ok(Box::new(CswapApp::new(cc, services, events)))
        }),
    )
}
