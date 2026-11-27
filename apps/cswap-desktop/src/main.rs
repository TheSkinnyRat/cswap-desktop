//! Thin app: single instance, tray, the window's lifetime, the platform [`cswap_core::Host`],
//! launch at login, notifications, and wiring driver + updater + UI together.
//! OWNER: platform.

fn main() -> anyhow::Result<()> {
    if let Some(outcome) = cswap_update::finish(&std::env::args_os().collect::<Vec<_>>()) {
        return outcome.map_err(anyhow::Error::msg);
    }
    Ok(())
}
