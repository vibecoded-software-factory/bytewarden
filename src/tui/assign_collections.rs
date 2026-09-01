use std::collections::HashSet;

use crate::domain::Collection;

#[derive(Debug, Clone)]
pub enum AssignCollectionsPurpose {
    UpdateField,

    MoveToOrg {
        item_id: String,
        organization_id: String,
    },
}

#[derive(Debug, Clone)]
pub struct AssignCollectionsState {
    pub available: Vec<Collection>,

    pub selected: HashSet<String>,

    pub cursor: usize,

    pub edit_field_idx: usize,

    pub origin: crate::tui::screens::Screen,

    pub purpose: AssignCollectionsPurpose,

    pub error: bool,
}

impl AssignCollectionsState {
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
        s.toggle_cursor();
        assert!(s.selected.contains("c1"));
        s.toggle_cursor();
        assert!(!s.selected.contains("c1"));
        s.cursor = 1;
        s.toggle_cursor();
        assert!(s.selected.contains("c2"));
    }

    #[test]
    fn collected_ids_preserves_available_list_order() {
        let mut s = build(vec![coll("c1", "Eng"), coll("c2", "Ops")], &[], 0);
        s.cursor = 1;
        s.toggle_cursor();
        s.cursor = 0;
        s.toggle_cursor();
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
