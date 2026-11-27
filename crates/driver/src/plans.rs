//! Plan labels (port of the I/O half of `src/main/plan.ts`; the label rule is
//! `cswap_core::plan`). cswap reports usage, never the subscription, but Claude Code's own
//! `.credentials.json` carries `subscriptionType` and `rateLimitTier` next to the tokens.
//! Exactly those two strings are read: the file is parsed into a struct that has no other
//! fields, so no token is held, returned, logged or cached. Only the live account's file
//! is readable (the rest are encrypted in the vault), so labels are remembered by email
//! in `plans.json` and survive a switch.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use cswap_core::PlanInfo;
use cswap_core::plan::label_for;
use serde::Deserialize;
use serde_json::Value;

use crate::resolve::home_dir;
use crate::store::write_atomic;
use crate::time::now_iso;

/// `$CLAUDE_CONFIG_DIR` (when set and not blank) or `~/.claude`.
pub fn claude_config_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CONFIG_DIR")
        .filter(|v| !v.to_string_lossy().trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| home_dir().map(|h| h.join(".claude")))
}

// Only these two fields exist here; serde skips every other key (the tokens) unread
// into any value.
#[derive(Deserialize)]
struct CredFile {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<OAuthPlan>,
}

#[derive(Deserialize)]
struct OAuthPlan {
    #[serde(rename = "subscriptionType")]
    subscription: Option<Value>,
    #[serde(rename = "rateLimitTier")]
    tier: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivePlan {
    pub tier: Option<String>,
    pub subscription: Option<String>,
    pub label: String,
}

pub fn read_active_plan(config_dir: &Path) -> Option<ActivePlan> {
    // macOS keeps these in the Keychain: a missing file is simply "unknown".
    let bytes = std::fs::read(config_dir.join(".credentials.json")).ok()?;
    let parsed = serde_json::from_slice::<CredFile>(&bytes);
    drop(bytes);
    let oauth = parsed.ok()?.oauth?;
    let s = |v: Option<Value>| v.and_then(|v| v.as_str().map(str::to_owned));
    let (tier, subscription) = (s(oauth.tier), s(oauth.subscription));
    let label = label_for(tier.as_deref(), subscription.as_deref())?;
    Some(ActivePlan { tier, subscription, label })
}

pub struct PlanStore {
    file: PathBuf,
    config_dir: Option<PathBuf>,
    data: BTreeMap<String, PlanInfo>,
}

impl PlanStore {
    pub fn open(data_dir: &Path, config_dir: Option<PathBuf>) -> Self {
        let file = data_dir.join("plans.json");
        // Entry by entry: one malformed record must not forget the others.
        let data = std::fs::read(&file)
            .ok()
            .and_then(|b| serde_json::from_slice::<BTreeMap<String, Value>>(&b).ok())
            .map(|m| m.into_iter().filter_map(|(k, v)| serde_json::from_value(v).ok().map(|p| (k, p))).collect())
            .unwrap_or_default();
        Self { file, config_dir, data }
    }

    pub fn all(&self) -> BTreeMap<String, PlanInfo> {
        self.data.clone()
    }

    /// After a refresh, with the account that is live: a plan is only ever recorded for
    /// the account the credential file belongs to. Returns whether anything changed.
    pub fn observe(&mut self, email: Option<&str>) -> bool {
        let Some(email) = email.filter(|e| !e.is_empty()) else { return false };
        let Some(plan) = self.config_dir.as_deref().and_then(read_active_plan) else { return false };
        if let Some(prev) = self.data.get(email)
            && prev.label == plan.label
            && prev.tier == plan.tier
        {
            return false;
        }
        self.data.insert(
            email.to_owned(),
            PlanInfo { label: plan.label, tier: plan.tier, subscription: plan.subscription, seen_at: now_iso() },
        );
        // Remembering is a convenience, never a failure.
        if let Ok(json) = serde_json::to_string_pretty(&self.data) {
            let _ = write_atomic(&self.file, json.as_bytes());
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_credentials(dir: &Path, oauth: Value) {
        std::fs::write(dir.join(".credentials.json"), serde_json::json!({ "claudeAiOauth": oauth }).to_string())
            .unwrap();
    }

    #[test]
    fn reads_the_two_plan_fields_and_no_others() {
        let d = tempfile::tempdir().unwrap();
        with_credentials(
            d.path(),
            serde_json::json!({"accessToken":"sk-secret","refreshToken":"sk-secret","subscriptionType":"max","rateLimitTier":"default_claude_max_20x"}),
        );
        let plan = read_active_plan(d.path()).unwrap();
        assert_eq!(
            plan,
            ActivePlan {
                tier: Some("default_claude_max_20x".into()),
                subscription: Some("max".into()),
                label: "Max 20x".into()
            }
        );
        assert!(!format!("{plan:?}").contains("secret"));
    }

    #[test]
    fn none_for_a_missing_unreadable_or_plan_less_file() {
        let d = tempfile::tempdir().unwrap();
        assert!(read_active_plan(&d.path().join("missing")).is_none());
        std::fs::write(d.path().join(".credentials.json"), "not json at all").unwrap();
        assert!(read_active_plan(d.path()).is_none());
        with_credentials(d.path(), serde_json::json!({"accessToken":"x"}));
        assert!(read_active_plan(d.path()).is_none());
        with_credentials(d.path(), serde_json::json!({"subscriptionType": 7}));
        assert!(read_active_plan(d.path()).is_none());
    }

    #[test]
    fn remembers_the_plan_per_email_and_survives_a_restart() {
        let home = tempfile::tempdir().unwrap();
        with_credentials(
            home.path(),
            serde_json::json!({"subscriptionType":"max","rateLimitTier":"default_claude_max_5x"}),
        );
        let mut store = PlanStore::open(home.path(), Some(home.path().into()));
        assert!(store.observe(Some("me@example.com")));
        assert_eq!(store.all()["me@example.com"].label, "Max 5x");
        assert!(!store.observe(Some("me@example.com")), "unchanged plan is not rewritten");
        // a different account is live now
        with_credentials(
            home.path(),
            serde_json::json!({"subscriptionType":"pro","rateLimitTier":"default_claude_pro"}),
        );
        store.observe(Some("other@example.com"));
        assert_eq!(store.all()["other@example.com"].label, "Pro");
        assert_eq!(store.all()["me@example.com"].label, "Max 5x");
        let reopened = PlanStore::open(home.path(), Some(home.path().into()));
        assert_eq!(reopened.all()["me@example.com"].label, "Max 5x");
    }

    #[test]
    fn records_nothing_without_an_active_account() {
        let home = tempfile::tempdir().unwrap();
        with_credentials(home.path(), serde_json::json!({"subscriptionType":"max"}));
        let mut store = PlanStore::open(home.path(), Some(home.path().into()));
        store.observe(None);
        store.observe(Some(""));
        assert!(store.all().is_empty());
    }

    #[test]
    fn a_bad_entry_in_plans_json_does_not_forget_the_rest() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(
            d.path().join("plans.json"),
            r#"{"a@example.com":{"label":"Pro","seenAt":"t"},"b@example.com":3}"#,
        )
        .unwrap();
        let s = PlanStore::open(d.path(), None);
        assert_eq!(s.all().len(), 1);
    }
}
