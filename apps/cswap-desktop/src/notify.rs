//! The desktop notification when auto-switch changes account (0.1.23 `core.ts`
//! `onAutoEvent`), when `notifyOnAutoSwitch` is on.

use cswap_core::AutoEvent;
use serde_json::Value;

/// Title and body for an auto event, or `None` when it is not a switch.
pub fn text(ev: &AutoEvent) -> Option<(String, String)> {
    if ev.event != "switch" {
        return None;
    }
    let dry = ev.extra.get("dryRun").and_then(Value::as_bool).unwrap_or(false);
    let title = if dry { "cswap would switch (dry run)" } else { "cswap switched account" };
    let trigger = match ev.extra.get("trigger") {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
    };
    let body = match ev.extra.get("to").filter(|v| v.is_object()) {
        Some(to) => {
            let n = to.get("number").and_then(Value::as_u64).map_or("?".to_string(), |n| n.to_string());
            let email = to.get("email").and_then(Value::as_str).unwrap_or("");
            format!("Now on Account-{n} ({email}) · {trigger}")
        }
        None => trigger,
    };
    Some((title.into(), body))
}

/// Shows it without blocking the caller (D-Bus / WinRT may take a moment).
pub fn show(title: String, body: String) {
    std::thread::spawn(move || {
        let mut n = notify_rust::Notification::new();
        n.summary(&title).body(&body).appname("Claude Swap");
        if let Err(e) = n.show() {
            tracing::warn!("notification failed: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(json: &str) -> AutoEvent {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn switch_events_read_as_0_1_23() {
        let e = ev(r#"{"schemaVersion":1,"event":"switch","ts":"t","to":{"number":3,"email":"c@example.com"},"trigger":"five_hour>=90"}"#);
        assert_eq!(text(&e).unwrap(), ("cswap switched account".into(), "Now on Account-3 (c@example.com) · five_hour>=90".into()));
        let e = ev(r#"{"event":"switch","dryRun":true,"to":{"email":"c@example.com"}}"#);
        assert_eq!(text(&e).unwrap(), ("cswap would switch (dry run)".into(), "Now on Account-? (c@example.com) · ".into()));
        let e = ev(r#"{"event":"switch","to":null,"trigger":"manual"}"#);
        assert_eq!(text(&e).unwrap().1, "manual");
        assert!(text(&ev(r#"{"event":"poll"}"#)).is_none());
    }
}
