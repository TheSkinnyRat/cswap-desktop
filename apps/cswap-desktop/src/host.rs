//! The [`Host`] the window talks to: native dialogs, opening paths and links, and asking
//! the shell to close or quit.
//!
//! Test seam as 0.1.23 (`ipc.ts stubbedDialog`): with `CSWAP_DESKTOP_TEST_DIALOGS` set to a
//! JSON file, dialogs answer from it instead of opening — `{ "directory", "openFile",
//! "saveFile", "binary" }`, a missing key or `null` meaning "cancelled". A file picker
//! without filters is the cswap-binary picker ("binary"), as in 0.1.23 where it was the
//! only one without them.

use std::path::PathBuf;
use std::sync::Arc;

use cswap_core::Host;

use crate::shell::Control;

pub const TEST_DIALOGS_ENV: &str = "CSWAP_DESKTOP_TEST_DIALOGS";

pub struct DesktopHost {
    pub control: Arc<Control>,
}

/// `None` = no seam, open the real dialog; `Some(answer)` otherwise.
fn stubbed(kind: &str) -> Option<Option<String>> {
    let f = std::env::var_os(TEST_DIALOGS_ENV).filter(|f| !f.is_empty())?;
    let answer = std::fs::read_to_string(f)
        .ok()
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v.get(kind).and_then(|a| a.as_str()).map(str::to_string));
    Some(answer)
}

fn path_string(p: PathBuf) -> String {
    p.to_string_lossy().into_owned()
}

impl Host for DesktopHost {
    fn pick_directory(&self, title: &str) -> Option<String> {
        if let Some(a) = stubbed("directory") {
            return a;
        }
        rfd::FileDialog::new().set_title(title).pick_folder().map(path_string)
    }

    fn pick_file(&self, title: &str, filters: &[(&str, &[&str])]) -> Option<String> {
        if let Some(a) = stubbed(if filters.is_empty() { "binary" } else { "openFile" }) {
            return a;
        }
        let mut d = rfd::FileDialog::new().set_title(title);
        for (name, exts) in filters {
            d = d.add_filter(*name, exts);
        }
        d.pick_file().map(path_string)
    }

    fn save_file(&self, title: &str, default_name: &str) -> Option<String> {
        if let Some(a) = stubbed("saveFile") {
            return a;
        }
        rfd::FileDialog::new().set_title(title).set_file_name(default_name).save_file().map(path_string)
    }

    fn open_path(&self, path: &str) {
        cswap_update::open_url(path);
    }

    fn open_external(&self, url: &str) {
        // Only web links: a `file:` or custom-scheme URL from a page must not start programs.
        if url.starts_with("https://") || url.starts_with("http://") {
            cswap_update::open_url(url);
        }
    }

    fn app_version(&self) -> String {
        env!("CARGO_PKG_VERSION").into()
    }

    fn platform(&self) -> &'static str {
        std::env::consts::OS
    }

    fn close_window(&self) {
        self.control.request_close();
    }

    fn quit(&self) {
        self.control.request_quit();
    }
}
