//! The window's root: caches pushed state, applies the theme, lays out the shell and the
//! current page. OWNER: ui-shell. The signature of [`CswapApp::new`] is the contract the
//! app crate builds against.

use std::sync::mpsc::Receiver;

use cswap_core::Event;

use crate::Services;

pub struct CswapApp {
    #[allow(dead_code)] // scaffold: ui-shell wires these up
    services: Services,
    #[allow(dead_code)]
    events: Receiver<Event>,
}

impl CswapApp {
    /// `events` carries everything pushed while this window is open; current state comes
    /// from the services' getters, because the app may build a fresh window each time.
    pub fn new(cc: &eframe::CreationContext<'_>, services: Services, events: Receiver<Event>) -> Self {
        let _ = cc;
        Self { services, events }
    }
}

impl eframe::App for CswapApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label("Claude Swap");
    }
}
