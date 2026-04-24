//! Tiny, zero-dependency template substitution for mock payloads.
//!
//! Tokens are delimited by `{{ }}` and are expanded recursively across
//! every string leaf of a `serde_json::Value`. Non-string values (numbers,
//! booleans, objects, arrays) pass through unchanged. Unknown tokens are
//! left in place so users can tell the substitution didn't fire, rather
//! than silently getting an empty string.
//!
//! Supported tokens:
//!
//! - `{{now}}` → current UTC time in RFC 3339 (e.g. `2026-04-24T12:34:56Z`).
//! - `{{now.epoch_ms}}` → current UTC time in ms since the Unix epoch.
//! - `{{uuid}}` → lowercase v4-shaped UUID.
//! - `{{random.int}}` → random integer in `0..1_000_000`.
//! - `{{random.int.<max>}}` → random integer in `0..<max>`.
//! - `{{path.<name>}}` → value of the matched path parameter.

use std::collections::BTreeMap;
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

/// Public entry point. Walks `value` and returns a new value with every
/// string leaf expanded. The original value is cloned; callers that
/// already own the value should consider whether they really need the
/// copy.
pub fn apply_templates(value: &Value, params: &BTreeMap<String, String>) -> Value {
    match value {
        Value::String(s) => Value::String(expand_string(s, params)),
        Value::Array(items) => Value::Array(
            items
                .iter()
                .map(|item| apply_templates(item, params))
                .collect(),
        ),
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(k.clone(), apply_templates(v, params));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

fn expand_string(input: &str, params: &BTreeMap<String, String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        rest = &rest[start + 2..];
        let Some(end) = rest.find("}}") else {
            // Unterminated token — leave the literal `{{...` as-is.
            out.push_str("{{");
            out.push_str(rest);
            return out;
        };
        let token = rest[..end].trim();
        let expanded = resolve_token(token, params);
        match expanded {
            Some(value) => out.push_str(&value),
            None => {
                // Unknown token: leave it in place so users can diagnose.
                out.push_str("{{");
                out.push_str(token);
                out.push_str("}}");
            }
        }
        rest = &rest[end + 2..];
    }
    out.push_str(rest);
    out
}

fn resolve_token(token: &str, params: &BTreeMap<String, String>) -> Option<String> {
    match token {
        "now" => Some(rfc3339_now()),
        "now.epoch_ms" => Some(epoch_ms_now().to_string()),
        "uuid" => Some(fake_uuid_v4()),
        "random.int" => Some(next_random_int(1_000_000).to_string()),
        _ => {
            if let Some(max_str) = token.strip_prefix("random.int.") {
                if let Ok(max) = max_str.parse::<u64>() {
                    let bound = max.max(1);
                    return Some(next_random_int(bound).to_string());
                }
                return None;
            }
            if let Some(name) = token.strip_prefix("path.") {
                return Some(params.get(name).cloned().unwrap_or_default());
            }
            if let Some(name) = token.strip_prefix("env.") {
                return Some(read_env_var(name));
            }
            None
        }
    }
}

/// Read an env var with a conservative allowlist. The gateway must never
/// surface `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GITHUB_TOKEN` or other
/// well-known secrets through a mock — someone sharing a reproduction
/// payload should not accidentally leak their credentials. Names matching
/// the deny list (case-insensitive) expand to an empty string and are
/// logged via the standard `{{env.NAME}}` unknown-token path is NOT used
/// (we want the substitution to happen, just with an empty value, so
/// downstream payload shape stays valid).
fn read_env_var(name: &str) -> String {
    if name.trim().is_empty() {
        return String::new();
    }
    if is_sensitive_env_name(name) {
        return String::new();
    }
    std::env::var(name).unwrap_or_default()
}

fn is_sensitive_env_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    // Allowlist deny substrings covering the common credential shapes —
    // API keys, tokens, secrets, passwords, cookies. We match on substring
    // (not exact) so `MY_API_KEY` and `OPENAI_API_KEY` both get blocked.
    const DENY_SUBSTRINGS: &[&str] = &[
        "SECRET",
        "PASSWORD",
        "PRIVATE_KEY",
        "API_KEY",
        "APIKEY",
        "TOKEN",
        "COOKIE",
        "AUTH",
        "CREDENTIAL",
    ];
    DENY_SUBSTRINGS.iter().any(|needle| upper.contains(needle))
}

fn rfc3339_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Minimal epoch → YYYY-MM-DDTHH:MM:SSZ conversion. Days-per-month
    // handling covers the Gregorian leap-year rule so the output is
    // accurate; no external chrono/time dependency.
    let (year, month, day, hour, minute, second) = epoch_to_civil(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn epoch_to_civil(secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as u32;
    let remainder = secs % 86_400;
    let hour = (remainder / 3_600) as u32;
    let minute = ((remainder % 3_600) / 60) as u32;
    let second = (remainder % 60) as u32;

    let mut year: u32 = 1970;
    let mut days_left = days;
    loop {
        let year_days = if is_leap_year(year) { 366 } else { 365 };
        if days_left < year_days {
            break;
        }
        days_left -= year_days;
        year += 1;
    }

    let mdays = month_lengths(year);
    let mut month: u32 = 1;
    for &len in &mdays {
        if days_left < len {
            break;
        }
        days_left -= len;
        month += 1;
    }
    let day = days_left + 1;
    (year, month, day, hour, minute, second)
}

fn is_leap_year(year: u32) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

fn month_lengths(year: u32) -> [u32; 12] {
    let feb = if is_leap_year(year) { 29 } else { 28 };
    [31, feb, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
}

fn epoch_ms_now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub(crate) fn fake_uuid_v4() -> String {
    // Not a cryptographic UUID — we just want a shape that looks correct
    // so mock consumers that validate the format (via regex) accept it.
    let a = next_random_int(0xFFFFFFFF);
    let b = next_random_int(0xFFFF);
    let c = next_random_int(0x0FFF) | 0x4000; // version 4
    let d = (next_random_int(0x3FFF) | 0x8000) & 0xBFFF; // variant 10xx
    let e_hi = next_random_int(0xFFFF);
    let e_lo = next_random_int(0xFFFFFFFF);
    format!("{a:08x}-{b:04x}-{c:04x}-{d:04x}-{e_hi:04x}{e_lo:08x}")
}

fn next_random_int(bound: u64) -> u64 {
    use std::cell::Cell;
    thread_local! {
        static STATE: Cell<u64> = Cell::new(seed());
    }
    STATE.with(|slot| {
        let mut x = slot.get();
        if x == 0 {
            x = seed();
        }
        x = x.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        slot.set(x);
        if bound == 0 {
            return 0;
        }
        (x >> 33) % bound
    })
}

fn seed() -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0) as u64;
    nanos ^ 0xA24BAED4963EE407
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn leaves_plain_strings_untouched() {
        let v = json!({"message": "hello"});
        assert_eq!(apply_templates(&v, &BTreeMap::new()), v);
    }

    #[test]
    fn substitutes_path_params() {
        let v = json!({"id": "{{path.id}}", "name": "Ada"});
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), "42".to_string());
        let out = apply_templates(&v, &params);
        assert_eq!(out["id"], "42");
        assert_eq!(out["name"], "Ada");
    }

    #[test]
    fn substitutes_unknown_params_with_empty() {
        let v = json!({"id": "{{path.missing}}"});
        let out = apply_templates(&v, &BTreeMap::new());
        assert_eq!(out["id"], "");
    }

    #[test]
    fn leaves_unknown_tokens_in_place() {
        let v = json!({"x": "{{wat.is.this}}"});
        let out = apply_templates(&v, &BTreeMap::new());
        assert_eq!(out["x"], "{{wat.is.this}}");
    }

    #[test]
    fn handles_mixed_literals_and_tokens() {
        let v = json!({"msg": "order {{path.id}} @ {{now.epoch_ms}}ms"});
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), "42".to_string());
        let out = apply_templates(&v, &params);
        let msg = out["msg"].as_str().unwrap();
        assert!(msg.starts_with("order 42 @ "));
        assert!(msg.ends_with("ms"));
    }

    #[test]
    fn generates_uuid_shaped_string() {
        let v = json!({"id": "{{uuid}}"});
        let out = apply_templates(&v, &BTreeMap::new());
        let id = out["id"].as_str().unwrap();
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5);
        assert_eq!(parts[0].len(), 8);
        assert_eq!(parts[1].len(), 4);
        assert_eq!(parts[2].len(), 4);
        assert_eq!(parts[3].len(), 4);
        assert_eq!(parts[4].len(), 12);
    }

    #[test]
    fn random_int_respects_bound() {
        let v = json!("{{random.int.10}}");
        for _ in 0..50 {
            let out = apply_templates(&v, &BTreeMap::new());
            let n: u64 = out.as_str().unwrap().parse().unwrap();
            assert!(n < 10, "{n}");
        }
    }

    #[test]
    fn now_looks_like_rfc3339() {
        let s = rfc3339_now();
        assert!(s.ends_with('Z'), "{s}");
        assert!(s.contains('T'));
        // Date portion should be exactly 10 chars.
        assert_eq!(s.split('T').next().unwrap().len(), 10);
    }

    #[test]
    fn env_token_resolves_to_env_var_value() {
        // Use a name not on the sensitive deny list. SAFETY: tests don't run
        // in parallel across threads that touch the env, and we clean up.
        unsafe {
            std::env::set_var("ALBERT_TEMPLATE_TEST_NAME", "canary");
        }
        let v = Value::String("hello {{env.ALBERT_TEMPLATE_TEST_NAME}}".to_string());
        let out = apply_templates(&v, &BTreeMap::new());
        assert_eq!(out.as_str().unwrap(), "hello canary");
        unsafe {
            std::env::remove_var("ALBERT_TEMPLATE_TEST_NAME");
        }
    }

    #[test]
    fn env_token_missing_var_expands_to_empty_string() {
        let v = Value::String("value=[{{env.ALBERT_TEMPLATE_DEFINITELY_UNSET}}]".to_string());
        let out = apply_templates(&v, &BTreeMap::new());
        assert_eq!(out.as_str().unwrap(), "value=[]");
    }

    #[test]
    fn env_token_refuses_sensitive_names_even_when_set() {
        // Redacted categories: anything matching SECRET / PASSWORD / TOKEN /
        // API_KEY / APIKEY / COOKIE / AUTH / CREDENTIAL / PRIVATE_KEY.
        // The gateway never surfaces these even when set.
        unsafe {
            std::env::set_var("ALBERT_TEMPLATE_API_KEY", "super-secret");
            std::env::set_var("ALBERT_TEMPLATE_AUTH_TOKEN", "bearer-xyz");
        }
        for name in ["ALBERT_TEMPLATE_API_KEY", "ALBERT_TEMPLATE_AUTH_TOKEN"] {
            let v = Value::String(format!("[{{{{env.{name}}}}}]"));
            let out = apply_templates(&v, &BTreeMap::new());
            assert_eq!(
                out.as_str().unwrap(),
                "[]",
                "sensitive env var leaked: {name}"
            );
        }
        unsafe {
            std::env::remove_var("ALBERT_TEMPLATE_API_KEY");
            std::env::remove_var("ALBERT_TEMPLATE_AUTH_TOKEN");
        }
    }

    #[test]
    fn is_sensitive_env_name_flags_common_secret_shapes() {
        assert!(is_sensitive_env_name("OPENAI_API_KEY"));
        assert!(is_sensitive_env_name("ANTHROPIC_API_KEY"));
        assert!(is_sensitive_env_name("GITHUB_TOKEN"));
        assert!(is_sensitive_env_name("MY_COOKIE_SECRET"));
        assert!(is_sensitive_env_name("DB_PASSWORD"));
        assert!(is_sensitive_env_name("basic_auth_user"));
        assert!(is_sensitive_env_name("TLS_PRIVATE_KEY_PATH"));
        assert!(!is_sensitive_env_name("BUILD_NUMBER"));
        assert!(!is_sensitive_env_name("HOSTNAME"));
        assert!(!is_sensitive_env_name("DEPLOY_ENV"));
    }
}
