//! Multi-select popup state for assigning an item to its
//! organisation's collections.
//!
//! Opened from the edit-mode "Collections" row via `Alt+L`. The
//! popup snapshots the org's visible collections, lets the user
//! toggle membership with `Space`, and on `Enter` validates that at
//! least one is selected (Bitwarden requires this for org items)
//! and copies the chosen UUIDs back into
//! [`crate::tui::edit_field::EditField::collection_ids`].

use std::collections::HashSet;

use crate::domain::Collection;

/// Discriminates how the popup was opened, which determines the
/// shape of the commit handler:
///
/// * `Edit` / `Create`: write the chosen UUIDs back into the
///   matching `EditField`. The actual `bw edit` / `bw create`
///   happens later via the regular Enter-on-form save flow.
/// * `Move`: call `bw move <item_id> <org_id> <ids>` directly.
///   The popup's `target_idx` is unused in this mode.
#[derive(Debug, Clone)]
pub enum AssignCollectionsPurpose {
    /// Update the `Collections` row of the edit / create form.
    UpdateField,
    /// Move a personal item into the org with the given UUID.
    /// Carries the `Item` UUID and the organisation UUID resolved
    /// at popup-open time so the commit can call `bw move`
    /// without rummaging through the rest of `App`.
    MoveToOrg {
        item_id: String,
        organization_id: String,
    },
}

/// Buffer for the in-flight popup. `None` outside the popup.
#[derive(Debug, Clone)]
pub struct AssignCollectionsState {
    /// Collections the user can pick from — pre-filtered to the
    /// item's owning organisation. Sorted alphabetically by display
    /// name so the cursor order is predictable.
    pub available: Vec<Collection>,
    /// UUIDs currently checked. Stored as a set so toggle is O(1)
    /// and order doesn't matter; on commit we materialise it as a
    /// `Vec<String>` matching `available`'s order for stable output.
    pub selected: HashSet<String>,
    /// Cursor index into `available`.
    pub cursor: usize,
    /// Index of the form row whose `collection_ids` we'll rewrite
    /// on commit. Points into `app.edit.fields` when origin =
    /// `Screen::Detail` (edit mode) or `app.create.fields` when
    /// origin = `Screen::Create`. Unused for the `MoveToOrg`
    /// purpose.
    pub edit_field_idx: usize,
    /// Screen the popup was opened from. Determines which field
    /// vector we write back to and where cancel/commit return the
    /// user.
    pub origin: crate::tui::screens::Screen,
    /// What the popup will do on commit.
    pub purpose: AssignCollectionsPurpose,
    /// `true` after a failed commit (no collection selected). The
    /// view shows an error strip until the user toggles something
    /// — clears on the next keypress.
    pub error: bool,
}

impl AssignCollectionsState {
    /// Builds a fresh popup state.
    ///
    /// `available` should already be filtered to the item's
    /// organisation and sorted by display name. `currently_selected`
    /// pre-checks the rows the item already belongs to.
    pub fn new(
        available: Vec<Collection>,
        currently_selected: &[String],
        edit_field_idx: usize,
        origin: crate::tui::screens::Screen,
        purpose: AssignCollectionsPurpose,
    ) -> Self {
        Self {
            selected: currently_selected.iter().cloned().collect(),
            cursor: 0,
            available,
            edit_field_idx,
            origin,
            purpose,
            error: false,
        }
    }

    /// Toggles the membership of the cursor row.
    pub fn toggle_cursor(&mut self) {
        let Some(row) = self.available.get(self.cursor) else {
            return;
        };
        if self.selected.contains(&row.id) {
            self.selected.remove(&row.id);
        } else {
            self.selected.insert(row.id.clone());
        }
    }

    /// Materialises the selection in display-list order, dropping
    /// any UUID whose collection is no longer visible (defensive —
    /// shouldn't happen in normal flow because we don't insert
    /// unknown ids).
    pub fn collected_ids(&self) -> Vec<String> {
        self.available
            .iter()
            .filter(|c| self.selected.contains(&c.id))
            .map(|c| c.id.clone())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn coll(id: &str, name: &str) -> Collection {
        Collection {
            id: id.into(),
            name: name.into(),
            organization_id: Some("o1".into()),
        }
    }

    fn build(avail: Vec<Collection>, sel: &[String], idx: usize) -> AssignCollectionsState {
        AssignCollectionsState::new(
            avail,
            sel,
            idx,
            crate::tui::screens::Screen::Detail,
            AssignCollectionsPurpose::UpdateField,
        )
    }

    #[test]
    fn new_pre_checks_currently_selected_ids() {
        let s = build(
            vec![coll("c1", "Eng"), coll("c2", "Ops")],
            &["c2".into()],
            0,
        );
        assert!(!s.selected.contains("c1"));
        assert!(s.selected.contains("c2"));
        assert_eq!(s.cursor, 0);
        assert!(!s.error);
    }

    #[test]
    fn toggle_cursor_flips_membership() {
        let mut s = build(vec![coll("c1", "Eng"), coll("c2", "Ops")], &[], 0);
        s.toggle_cursor(); // c1 → on
        assert!(s.selected.contains("c1"));
        s.toggle_cursor(); // c1 → off
        assert!(!s.selected.contains("c1"));
        s.cursor = 1;
        s.toggle_cursor(); // c2 → on
        assert!(s.selected.contains("c2"));
    }

    #[test]
    fn collected_ids_preserves_available_list_order() {
        // Even if the user toggled c2 first then c1, the output
        // should follow `available` order so saves are deterministic.
        let mut s = build(vec![coll("c1", "Eng"), coll("c2", "Ops")], &[], 0);
        s.cursor = 1;
        s.toggle_cursor(); // c2
        s.cursor = 0;
        s.toggle_cursor(); // c1
        assert_eq!(s.collected_ids(), vec!["c1".to_string(), "c2".to_string()]);
    }

    #[test]
    fn toggle_cursor_out_of_range_is_a_noop() {
        let mut s = build(vec![coll("c1", "Eng")], &[], 0);
        s.cursor = 99;
        s.toggle_cursor();
        assert!(s.selected.is_empty());
    }

    #[test]
    fn move_purpose_carries_target_ids() {
        // Smoke test that the new variant constructs and exposes
        // the captured ids — the flow's commit branch reads them.
        let s = AssignCollectionsState::new(
            vec![coll("c1", "Eng")],
            &[],
            0,
            crate::tui::screens::Screen::Detail,
            AssignCollectionsPurpose::MoveToOrg {
                item_id: "i1".into(),
                organization_id: "o1".into(),
            },
        );
        match &s.purpose {
            AssignCollectionsPurpose::MoveToOrg {
                item_id,
                organization_id,
            } => {
                assert_eq!(item_id, "i1");
                assert_eq!(organization_id, "o1");
            }
            _ => panic!("expected MoveToOrg purpose"),
        }
    }
}
