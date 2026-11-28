//! The Claude Swap window. Pages talk only to the [`cswap_core::api`] traits, so the same
//! code runs against the real driver, the in-memory mock (tests, README screenshots) and
//! nothing platform-specific: the app crate owns the event loop, the tray and the
//! window's lifetime (it may build a fresh [`CswapApp`] each time the window opens, so all
//! state the user would miss must come back from the services' getters).
//!
//! CONTRACT: [`Services`], [`CswapApp::new`], [`cx::Cx`], [`theme::Tokens`] and the page
//! shape (`new` / `on_event` / `ui`). OWNERS: ui-shell owns `app`, `chrome`, `shell`, `widgets`,
//! `palette`, `style`, `testing`, `toast` rendering and `pages::{settings, log, onboarding}`; ui-accounts owns
//! `usage` and `pages::{accounts, auto, mappings}`; `theme` and `cx` are the main session's.

use std::sync::Arc;

use cswap_core::{Cswap, Host, Updates};

pub mod app;
pub mod chrome;
pub mod cx;
pub mod pages;
pub mod palette;
pub mod shell;
pub mod style;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod theme;
pub mod toast;
pub mod usage;
pub mod widgets;

pub use app::CswapApp;
pub use chrome::WindowChrome;

/// The services a window talks to.
#[derive(Clone)]
pub struct Services {
    pub cswap: Arc<dyn Cswap>,
    pub updates: Arc<dyn Updates>,
    pub host: Arc<dyn Host>,
}

#[cfg(test)]
mod tests;
