//! JSON helpers shared by the `bw` adapter.

use serde_json::Value;

/// Extracts a non-empty string field from a [`Value`].
///
/// Returns `None` when the key is absent, the value is not a string, or
/// the string is empty.
pub fn opt_str(val: &Value, key: &str) -> Option<String> {
    val[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn returns_some_for_non_empty_string() {
        let v = json!({"name": "alice"});
        assert_eq!(opt_str(&v, "name"), Some("alice".to_string()));
    }

    #[test]
    fn returns_none_for_missing_key() {
        let v = json!({"other": "x"});
        assert_eq!(opt_str(&v, "missing"), None);
    }

    #[test]
    fn returns_none_for_empty_string() {
        let v = json!({"name": ""});
        assert_eq!(opt_str(&v, "name"), None);
    }

    #[test]
    fn returns_none_for_non_string_types() {
        let v = json!({"count": 42, "flag": true, "n": null});
        assert_eq!(opt_str(&v, "count"), None);
        assert_eq!(opt_str(&v, "flag"), None);
        assert_eq!(opt_str(&v, "n"), None);
    }
}
