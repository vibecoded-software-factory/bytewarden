//! Folder-filter state for the sidebar.

use crate::domain::Folder;

/// Folder filter applied to the vault list, ANDed with the active
/// item-type filter ([`crate::domain::filter::ItemFilter`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderFilter {
    /// Show items from every folder (no folder constraint).
    All,
    /// Show only items that have no folder assigned (`folder_id` is
    /// `None`).
    NoFolder,
    /// Show only items whose `folder_id` matches the given UUID.
    Folder(String),
}

impl FolderFilter {
    /// Returns `true` if the given `folder_id` matches the filter.
    pub fn matches(&self, folder_id: Option<&str>) -> bool {
        match self {
            FolderFilter::All => true,
            FolderFilter::NoFolder => folder_id.is_none(),
            FolderFilter::Folder(id) => folder_id == Some(id.as_str()),
        }
    }
}

/// Returns the rendered label for the sidebar row at `idx`.
///
/// The sidebar order is:
/// * `0` — "All folders"
/// * `1` — "(No folder)"
/// * `2..` — one row per folder, alphabetical by name.
pub fn row_label(idx: usize, folders: &[Folder]) -> Option<&str> {
    match idx {
        0 => Some("All folders"),
        1 => Some("(No folder)"),
        _ => folders.get(idx - 2).map(|f| f.name.as_str()),
    }
}

/// Total number of rows in the sidebar (2 fixed + one per folder).
pub fn row_count(folders: &[Folder]) -> usize {
    2 + folders.len()
}

/// Resolves a sidebar row index to the [`FolderFilter`] it represents.
pub fn filter_for_row(idx: usize, folders: &[Folder]) -> FolderFilter {
    match idx {
        0 => FolderFilter::All,
        1 => FolderFilter::NoFolder,
        _ => folders
            .get(idx - 2)
            .map(|f| FolderFilter::Folder(f.id.clone()))
            .unwrap_or(FolderFilter::All),
    }
}

/// Resolves a [`FolderFilter`] back to its sidebar row index, used to
/// keep the highlight in sync after a folder list reload.
pub fn row_for_filter(filter: &FolderFilter, folders: &[Folder]) -> usize {
    match filter {
        FolderFilter::All => 0,
        FolderFilter::NoFolder => 1,
        FolderFilter::Folder(id) => folders
            .iter()
            .position(|f| &f.id == id)
            .map(|i| i + 2)
            .unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folders() -> Vec<Folder> {
        vec![
            Folder {
                id: "f1".into(),
                name: "Work".into(),
            },
            Folder {
                id: "f2".into(),
                name: "Personal".into(),
            },
        ]
    }

    #[test]
    fn all_matches_any_folder_id() {
        let f = FolderFilter::All;
        assert!(f.matches(None));
        assert!(f.matches(Some("f1")));
        assert!(f.matches(Some("anything")));
    }

    #[test]
    fn no_folder_matches_only_none() {
        let f = FolderFilter::NoFolder;
        assert!(f.matches(None));
        assert!(!f.matches(Some("f1")));
    }

    #[test]
    fn specific_folder_matches_only_that_id() {
        let f = FolderFilter::Folder("f1".into());
        assert!(f.matches(Some("f1")));
        assert!(!f.matches(Some("f2")));
        assert!(!f.matches(None));
    }

    #[test]
    fn row_count_includes_two_meta_rows() {
        assert_eq!(row_count(&[]), 2);
        assert_eq!(row_count(&folders()), 4);
    }

    #[test]
    fn row_label_meta_then_folders() {
        let fs = folders();
        assert_eq!(row_label(0, &fs), Some("All folders"));
        assert_eq!(row_label(1, &fs), Some("(No folder)"));
        assert_eq!(row_label(2, &fs), Some("Work"));
        assert_eq!(row_label(3, &fs), Some("Personal"));
        assert_eq!(row_label(4, &fs), None);
    }

    #[test]
    fn filter_for_row_round_trips_back_via_row_for_filter() {
        let fs = folders();
        for idx in 0..row_count(&fs) {
            let filter = filter_for_row(idx, &fs);
            assert_eq!(row_for_filter(&filter, &fs), idx);
        }
    }

    #[test]
    fn filter_for_row_out_of_range_falls_back_to_all() {
        let fs = folders();
        assert_eq!(filter_for_row(99, &fs), FolderFilter::All);
    }

    #[test]
    fn row_for_filter_unknown_folder_id_falls_back_to_zero() {
        let fs = folders();
        let unknown = FolderFilter::Folder("missing-uuid".into());
        assert_eq!(row_for_filter(&unknown, &fs), 0);
    }
}
