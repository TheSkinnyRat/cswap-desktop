//! cswap's `--json` schema v1 as the app sees it, plus the app-side records the UI shows.
//! Mirrors `src/shared/types.ts` of 0.1.23. Upstream adds fields freely, so every cswap
//! object keeps what it does not know in `extra`: dropping them would lose data on a
//! round-trip and hide new upstream fields from the LOG page.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// Unknown keys of a cswap object, kept verbatim.
pub type Extra = Map<String, Value>;

/// `usageStatus` is an open set upstream (`ok`, `token_expired`, `api_key`,
/// `keychain_unavailable`, `relogin_required`, `foreign_credential`, `no_credentials`,
/// `unavailable`, ...), so it stays a string.
pub type UsageStatus = String;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub pct: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub countdown: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub clock: Option<String>,
    /// Weekly windows only, once the week is about a day old.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_pct: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ahead_of_pace: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub projected_exhaustion_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub will_last_to_reset: Option<bool>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedWindow {
    pub name: String,
    #[serde(flatten)]
    pub window: UsageWindow,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub five_hour: Option<UsageWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seven_day: Option<UsageWindow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scoped: Option<Vec<ScopedWindow>>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub number: u32,
    pub email: String,
    #[serde(default)]
    pub organization_name: String,
    #[serde(default)]
    pub organization_uuid: String,
    #[serde(default)]
    pub is_organization: bool,
    #[serde(default)]
    pub active: bool,
    #[serde(default)]
    pub usage_status: UsageStatus,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_fetched_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_age_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_good_usage: Option<Usage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_good_fetched_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_good_age_seconds: Option<f64>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListPayload {
    pub schema_version: u32,
    #[serde(default)]
    pub active_account_number: Option<u32>,
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountRef {
    #[serde(default)]
    pub number: Option<u32>,
    #[serde(default)]
    pub email: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SwitchPayload {
    pub schema_version: u32,
    #[serde(default)]
    pub switched: bool,
    #[serde(default)]
    pub from: Option<AccountRef>,
    #[serde(default)]
    pub to: Option<AccountRef>,
    #[serde(default)]
    pub strategy: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub message: String,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub models: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_source: Option<String>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// `status --json`'s `active`: a partial account plus whether cswap manages it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusActive {
    pub email: String,
    #[serde(default)]
    pub managed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub number: Option<u32>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPayload {
    pub schema_version: u32,
    #[serde(default)]
    pub active: Option<StatusActive>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_managed_accounts: Option<u32>,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigSetting {
    pub key: String,
    #[serde(default)]
    pub value: Value,
    #[serde(default)]
    pub is_set: bool,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPayload {
    pub schema_version: u32,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub settings: Vec<ConfigSetting>,
    #[serde(flatten)]
    pub extra: Extra,
}

/// cswap's `{"error":{"type","message"}}`, and the app's own errors in the same shape
/// (`NotFound`, `LaunchError`, `Timeout`, `Spawn`, ...).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CswapError {
    #[serde(rename = "type")]
    pub kind: String,
    pub message: String,
}

impl CswapError {
    pub fn new(kind: impl Into<String>, message: impl Into<String>) -> Self {
        Self { kind: kind.into(), message: message.into() }
    }
}

impl std::fmt::Display for CswapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for CswapError {}

pub type CsResult<T> = Result<T, CswapError>;

/// One line of `cswap auto --json`. Kinds are additive upstream (`poll`, `switch`,
/// `no-switch`, `account-quarantined`, `account-unquarantined`, `all-exhausted`, `sleep`,
/// `error`, `config-warning`, ...), so `event` stays a string and the rest stays a map.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoEvent {
    #[serde(default)]
    pub schema_version: u32,
    pub event: String,
    #[serde(default)]
    pub ts: String,
    #[serde(flatten)]
    pub extra: Extra,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoStatus {
    pub running: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AutoOutcome {
    Switched,
    Error,
    NoAction,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoOnceResult {
    pub exit_code: Option<i32>,
    pub outcome: AutoOutcome,
    pub events: Vec<AutoEvent>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Mapping {
    pub path: String,
    pub email: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_uuid: Option<String>,
    /// Joined in by the driver from the current list, when the email matches one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<Account>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnclaimedEntry {
    pub id: String,
    pub slot: String,
    pub reason: String,
}

/// A single CLI invocation as the app saw it; the LOG page shows these. Secrets never
/// reach `args` (tokens go through stdin).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandLogEntry {
    pub id: u64,
    pub at: String,
    pub args: Vec<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdaterStatus {
    #[default]
    Disabled,
    Idle,
    Checking,
    Available,
    Downloading,
    Downloaded,
    NotAvailable,
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterState {
    pub status: UpdaterStatus,
    pub current: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checked_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
    /// Why updating is off for this copy (dev build, unwritable install, translocated
    /// macOS bundle, ...), shown instead of the check button.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BinarySource {
    Settings,
    Env,
    Uv,
    Pipx,
    Path,
    #[default]
    None,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CswapBinaryInfo {
    pub path: Option<String>,
    pub source: BinarySource,
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub candidates: Vec<String>,
}

/// The plan label remembered per account email (`plans.json` in the app's data dir):
/// read from Claude Code's live credential file while that account was active, so a
/// switched-away account keeps its last known plan.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanInfo {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subscription: Option<String>,
    pub seen_at: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsState {
    pub payload: Option<ListPayload>,
    pub fetched_at: Option<String>,
    pub refreshing: bool,
    pub error: Option<CswapError>,
    #[serde(default)]
    pub plans: std::collections::BTreeMap<String, PlanInfo>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportOptions {
    pub account: Option<String>,
    pub full: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportOptions {
    pub force: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AddAccountOptions {
    pub slot: Option<u32>,
    pub alias: Option<String>,
    pub overwrite: bool,
}

#[derive(Clone, Default, PartialEq, Eq)]
pub struct AddTokenOptions {
    pub token: String,
    pub email: Option<String>,
    pub slot: Option<u32>,
    pub overwrite: bool,
}

// Hand-written so a token never lands in a log line through `{:?}`.
impl std::fmt::Debug for AddTokenOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AddTokenOptions")
            .field("token", &"<redacted>")
            .field("email", &self.email)
            .field("slot", &self.slot)
            .field("overwrite", &self.overwrite)
            .finish()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlainOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotateStrategy {
    Best,
    NextAvailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchedSession {
    pub command: String,
    pub cwd: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transfer {
    pub path: String,
    pub output: PlainOutput,
}

/// Pages the sidebar, the command palette and the tray can open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Page {
    Accounts,
    Auto,
    Mappings,
    Settings,
    Log,
}
