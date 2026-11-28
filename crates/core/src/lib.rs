//! Pure model of Claude Swap. No I/O, no threads of its own, no platform code: everything
//! here builds and tests the same on Windows, macOS and Linux.
//!
//! CONTRACT. `types`, `settings` and `api` are what the other crates compile against
//! (driver implements [`api::Cswap`], update implements [`api::Updates`], the app implements
//! [`api::Host`], the UI consumes all three). Change a signature only after telling the main
//! session. `format`, `plan`, `view` and `mock` are owned by the parts named in each file.

pub mod api;
pub mod format;
pub mod mock;
pub mod plan;
pub mod ready;
pub mod settings;
pub mod types;
pub mod view;

pub use api::{Cswap, Event, EventSink, Host, Pending, Updates, Waker};
pub use settings::{AccountsLayout, AppSettings, ThemeSetting};
pub use types::*;
