//! Lightweight fuzzy ranking used by the vault search box.
//!
//! The algorithm is deliberately simple — name match wins, then prefix bonus,
//! subsequence match, username match, URI match, notes match. It is enough
//! for a vault size of a few thousand items and runs purely on local data.

use crate::domain::item::Item;

/// Computes a relevance score between an item and a lower-cased query.
///
/// A score of `0` means "no match" and the caller should drop the item.
/// Scores are unitless: only the relative ordering between items matters.
///
/// # Examples
///
/// ```
/// // Name prefix matches outrank substring-only matches.
/// // (See unit tests in this module for concrete examples.)
/// ```
pub fn fuzzy_score(item: &Item, query: &str) -> i32 {
    let name = item.name.to_lowercase();
    let mut score = 0i32;

    if name.contains(query) {
        score += 100;
        if name.starts_with(query) {
            score += 20;
        }
    } else if is_subseq(query, &name) {
        score += 50;
    }

    if let Some(login) = &item.login {
        if let Some(u) = &login.username {
            let u = u.to_lowercase();
            if u.contains(query) {
                score += 30;
            } else if is_subseq(query, &u) {
                score += 10;
            }
        }
        for u in login.uris.iter().flatten() {
            if let Some(uri) = &u.uri
                && uri.to_lowercase().contains(query)
            {
                score += 10;
                break;
            }
        }
    }

    if let Some(notes) = &item.notes
        && notes.to_lowercase().contains(query)
    {
        score += 5;
    }

    score
}

/// Returns `true` if every character of `needle` appears in `haystack`
/// in the same order (not necessarily contiguous).
///
/// Example: `is_subseq("abc", "axbycz") == true`.
fn is_subseq(needle: &str, haystack: &str) -> bool {
    let mut chars = haystack.chars();
    needle.chars().all(|c| chars.any(|h| h == c))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::item::{Item, LoginData, UriData};

    #[test]
    fn subseq_matches_in_order() {
        assert!(is_subseq("abc", "axbycz"));
    }

    #[test]
    fn subseq_rejects_out_of_order() {
        assert!(!is_subseq("cba", "axbycz"));
    }

    #[test]
    fn empty_needle_always_matches() {
        assert!(is_subseq("", "anything"));
    }

    fn item(name: &str) -> Item {
        Item {
            id: "id".into(),
            name: name.into(),
            item_type: 1,
            login: None,
            card: None,
            identity: None,
            ssh_key: None,
            notes: None,
            folder_id: None,
            favorite: false,
            fields: vec![],
            attachments: None,
        }
    }

    #[test]
    fn name_substring_scores_100() {
        let i = item("My GitHub Login");
        assert_eq!(fuzzy_score(&i, "github"), 100);
    }

    #[test]
    fn name_prefix_adds_bonus_above_substring() {
        let prefix = item("Github thing");
        let middle = item("My Github thing");
        assert!(fuzzy_score(&prefix, "github") > fuzzy_score(&middle, "github"));
        assert_eq!(fuzzy_score(&prefix, "github"), 120);
    }

    #[test]
    fn subsequence_match_scores_50_when_no_substring() {
        // 'g','h','b' all appear in order in "GitHub" but not contiguous.
        let i = item("GitHub");
        assert_eq!(fuzzy_score(&i, "ghb"), 50);
    }

    #[test]
    fn no_match_returns_zero() {
        let i = item("hello");
        assert_eq!(fuzzy_score(&i, "xyz"), 0);
    }

    #[test]
    fn username_substring_adds_30() {
        let mut i = item("anything");
        i.login = Some(LoginData {
            username: Some("alice@example.com".into()),
            password: None,
            uris: None,
            totp: None,
        });
        // Name fails ("anything" vs "alice"): no substring, no subseq.
        // Username "alice@example.com" contains "alice" → +30.
        assert_eq!(fuzzy_score(&i, "alice"), 30);
    }

    #[test]
    fn uri_substring_adds_10() {
        let mut i = item("anything");
        i.login = Some(LoginData {
            username: None,
            password: None,
            uris: Some(vec![UriData {
                uri: Some("https://github.com/me".into()),
                match_type: None,
            }]),
            totp: None,
        });
        assert_eq!(fuzzy_score(&i, "github.com"), 10);
    }

    #[test]
    fn notes_substring_adds_5() {
        let mut i = item("anything");
        i.notes = Some("a really cool secret about widgets".into());
        assert_eq!(fuzzy_score(&i, "widgets"), 5);
    }

    #[test]
    fn name_and_username_scores_combine() {
        let mut i = item("My GitHub Login");
        i.login = Some(LoginData {
            username: Some("github-user".into()),
            password: None,
            uris: None,
            totp: None,
        });
        // 100 (substring) + 30 (username substring).
        assert_eq!(fuzzy_score(&i, "github"), 130);
    }
}
