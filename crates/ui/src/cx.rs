//! What a page gets each frame. Built by `app` from its cached state; pages read from it
//! and push intents (toasts, navigation) into it instead of reaching into the shell.

use cswap_core::{AccountsState, AppSettings, AutoStatus, Page};

use crate::Services;
use crate::theme::Tokens;
use crate::toast::Toasts;

pub struct Cx<'a> {
    pub services: &'a Services,
    pub tokens: &'a Tokens,
    pub settings: &'a AppSettings,
    pub accounts: &'a AccountsState,
    pub auto: &'a AutoStatus,
    pub toasts: &'a mut Toasts,
    /// Set to open another page after this frame.
    pub navigate: &'a mut Option<Page>,
}
