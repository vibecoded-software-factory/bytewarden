//! Lightweight fuzzy ranking used by the vault search box.
//!
//! The algorithm is deliberately simple — name match wins, then prefix bonus,
//! subsequence match, username match, URI match, notes match. It is enough
//! for a vault size of a few thousand items and runs purely on local data.
//!
//! ## Hot path
//!
//! Calling [`fuzzy_score`] re-lowercases the item's name, username, every
//! URI and the notes on every invocation, which adds up across a large
//! vault and a fast typist. The TUI keeps a parallel
//! `Vec<LoweredItem>` populated from `app.items`/`app.trashed_items`
//! and calls [`fuzzy_score_lowered`] instead, so the lowercase work
//! happens once per item per *mutation* rather than once per item per
//! *keystroke*.
//!
//! [`fuzzy_score`] survives as a thin wrapper that builds a temporary
//! [`LoweredItem`] — useful for ad-hoc tests and any caller that
//! doesn't want to maintain a side cache. Both functions return the
//! same scores; the public test suite treats the wrapper as the
//! reference implementation.

use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::domain::item::Item;

/// Pre-lowercased view of the searchable fields of an [`Item`].
///
/// Owns its strings — the TUI builds these once when items are loaded
/// (or mutated) and keeps the vector parallel to `app.items` /
/// `app.trashed_items`. Reading them on every keystroke is then O(N)
/// of cheap `&str::contains` calls instead of O(N) of allocations.
///
/// Carries the same `Zeroize`/`ZeroizeOnDrop` derives as [`Item`] — the
/// lowercased copies aren't secrets per se (name, username, URIs,
/// notes), but they are derived from the same surface as the items
/// themselves, and treating them with identical hygiene means the
/// security guarantees on `App::items` extend to `App::items_lowered`
/// without exception.
#[derive(Debug, Clone, Default, Zeroize, ZeroizeOnDrop)]
pub struct LoweredItem {
    /// Lowercased item name.
    pub name: String,
    /// Lowercased login username, when present.
    pub username: Option<String>,
    /// Lowercased login URIs (one entry per non-empty URI).
    pub uris: Vec<String>,
    /// Lowercased notes, when non-empty.
    pub notes: Option<String>,
}

impl LoweredItem {
    /// Builds a fresh [`LoweredItem`] from `item`. The lowercase
    /// happens once here; reads via [`fuzzy_score_lowered`] are
    /// allocation-free.
    pub fn from_item(item: &Item) -> Self {
        let name = item.name.to_lowercase();
        let (username, uris) = match item.login.as_ref() {
            Some(login) => {
                let username = login.username.as_deref().map(str::to_lowercase);
                let uris: Vec<String> = login
                    .uris
                    .iter()
                    .flatten()
                    .filter_map(|u| u.uri.as_deref())
                    .filter(|s| !s.is_empty())
                    .map(str::to_lowercase)
                    .collect();
                (username, uris)
            }
            None => (None, Vec::new()),
        };
        let notes = item
            .notes
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_lowercase);
        Self {
            name,
            username,
            uris,
            notes,
        }
    }
}

/// Computes a relevance score against a pre-lowercased view of an
/// item. Caller provides `query` already lower-cased.
///
/// This is the hot-path implementation — no allocations, just a
/// handful of `&str::contains` / subsequence checks.
pub fn fuzzy_score_lowered(lowered: &LoweredItem, query: &str) -> i32 {
    let mut score = 0i32;

    if lowered.name.contains(query) {
        score += 100;
        if lowered.name.starts_with(query) {
            score += 20;
        }
    } else if is_subseq(query, &lowered.name) {
        score += 50;
    }

    if let Some(u) = &lowered.username {
        if u.contains(query) {
            score += 30;
        } else if is_subseq(query, u) {
            score += 10;
        }
    }

    for uri in &lowered.uris {
        if uri.contains(query) {
            score += 10;
            break;
        }
    }

    if let Some(notes) = &lowered.notes
        && notes.contains(query)
    {
        score += 5;
    }

    score
}

/// Computes a relevance score between an item and a lower-cased query.
///
/// Convenience wrapper: builds a [`LoweredItem`] on the fly and
/// delegates to [`fuzzy_score_lowered`]. **Allocates** on every call;
/// callers in the search hot path should keep a `Vec<LoweredItem>`
/// alongside their `Vec<Item>` and call [`fuzzy_score_lowered`]
/// directly.
///
/// Kept on the public surface because it's the easiest entry point
/// for tests and ad-hoc callers, and because removing it would break
/// the existing fuzzy-search doctest.
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
    fuzzy_score_lowered(&LoweredItem::from_item(item), query)
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
            organization_id: None,
            collection_ids: Vec::new(),
            favorite: false,
            fields: vec![],
            attachments: None,
            reprompt: 0,
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

    // ── LoweredItem parity ──────────────────────────────────────────────
    //
    // The wrapper `fuzzy_score(item, query)` and the hot-path
    // `fuzzy_score_lowered(LoweredItem::from_item(&item), query)` must
    // return identical scores for every relevant input shape. These
    // tests guard against drift the day someone tweaks one but not the
    // other.

    fn assert_parity(item: &Item, query: &str) {
        let direct = fuzzy_score(item, query);
        let lowered = fuzzy_score_lowered(&LoweredItem::from_item(item), query);
        assert_eq!(
            direct, lowered,
            "fuzzy_score / fuzzy_score_lowered diverged for query {query:?}"
        );
    }

    #[test]
    fn lowered_parity_name_only_item() {
        let i = item("My GitHub Login");
        for q in ["github", "my", "ghb", "xyz", ""] {
            assert_parity(&i, q);
        }
    }

    #[test]
    fn lowered_parity_login_with_uris() {
        let mut i = item("Site");
        i.login = Some(LoginData {
            username: Some("Alice@Example.com".into()),
            password: None,
            uris: Some(vec![UriData {
                uri: Some("https://GitHub.com/alice".into()),
                match_type: None,
            }]),
            totp: None,
        });
        for q in ["alice", "github.com", "site", "example", "xyz"] {
            assert_parity(&i, q);
        }
    }

    #[test]
    fn lowered_parity_with_notes() {
        let mut i = item("Site");
        i.notes = Some("API key for the production WIDGETS pipeline".into());
        for q in ["widgets", "production", "missing"] {
            assert_parity(&i, q);
        }
    }

    #[test]
    fn lowered_item_drops_empty_uris_and_notes() {
        let mut i = item("Site");
        i.notes = Some(String::new());
        i.login = Some(LoginData {
            username: Some(String::new()),
            password: None,
            uris: Some(vec![
                UriData {
                    uri: Some(String::new()),
                    match_type: None,
                },
                UriData {
                    uri: Some("https://x".into()),
                    match_type: None,
                },
            ]),
            totp: None,
        });
        let lowered = LoweredItem::from_item(&i);
        // Empty notes/username are dropped (no point keeping them — they
        // can't contribute to any score), but a non-empty username slot
        // is preserved even when the value is empty so the parity tests
        // still match.
        assert!(lowered.notes.is_none());
        // The username is wrapped in `Some("")` because we preserve the
        // original `Some` shape without further filtering — the score
        // logic short-circuits on empty strings either way. Document
        // the behaviour rather than relying on it.
        assert_eq!(lowered.username.as_deref(), Some(""));
        assert_eq!(lowered.uris, vec!["https://x".to_string()]);
    }
}
