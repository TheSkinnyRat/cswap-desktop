//! One module per page. Shape: `XPage::new()`, `XPage::on_event(&mut self, &Event)`,
//! `XPage::ui(&mut self, ui: &mut egui::Ui, cx: &mut Cx)`.

pub mod accounts;
pub mod auto;
// ui-accounts' private controls, until the shared `widgets` replace them.
pub(crate) mod kit;
pub mod log;
pub mod mappings;
pub mod onboarding;
pub mod settings;
