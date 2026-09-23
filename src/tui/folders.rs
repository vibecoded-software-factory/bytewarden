use crate::domain::{Collection, Folder};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FolderFilter {
    All,

    NoFolder,

    Folder(String),

    Collection(String),
}

impl FolderFilter {
    pub fn matches(&self, folder_id: Option<&str>, collection_ids: &[String]) -> bool {
        match self {
            FolderFilter::All => true,
            FolderFilter::NoFolder => folder_id.is_none(),
            FolderFilter::Folder(id) => folder_id == Some(id.as_str()),
            FolderFilter::Collection(id) => collection_ids.iter().any(|c| c == id),
        }
    }
}

pub fn row_label(
    idx: usize,
    folders: &[Folder],
    collections: &[Collection],
    organizations: &[crate::domain::Organization],
) -> Option<String> {
    let folder_rows = 2 + folders.len();
    match idx {
        0 => Some("All folders".to_string()),
        1 => Some("(No folder)".to_string()),
        i if i < folder_rows => folders.get(i - 2).map(|f| f.name.clone()),
        i => {
            let coll_idx = i - folder_rows;
            collections.get(coll_idx).map(|c| {
                let org = c
                    .organization_id
                    .as_deref()
                    .and_then(|id| organizations.iter().find(|o| o.id == id))
                    .map(|o| o.name.as_str());
                match org {
                    Some(org_name) => format!("{org_name} / {}", c.name),
                    None => c.name.clone(),
                }
            })
        }
    }
}

pub fn row_count(folders: &[Folder], collections: &[Collection]) -> usize {
    2 + folders.len() + collections.len()
}

pub fn filter_for_row(idx: usize, folders: &[Folder], collections: &[Collection]) -> FolderFilter {
    let folder_rows = 2 + folders.len();
    match idx {
        0 => FolderFilter::All,
        1 => FolderFilter::NoFolder,
        i if i < folder_rows => folders
            .get(i - 2)
            .map(|f| FolderFilter::Folder(f.id.clone()))
            .unwrap_or(FolderFilter::All),
        i => collections
            .get(i - folder_rows)
            .map(|c| FolderFilter::Collection(c.id.clone()))
            .unwrap_or(FolderFilter::All),
    }
}

pub fn row_for_filter(
    filter: &FolderFilter,
    folders: &[Folder],
    collections: &[Collection],
) -> usize {
    match filter {
        FolderFilter::All => 0,
        FolderFilter::NoFolder => 1,
        FolderFilter::Folder(id) => folders
            .iter()
            .position(|f| &f.id == id)
            .map(|i| i + 2)
            .unwrap_or(0),
        FolderFilter::Collection(id) => collections
            .iter()
            .position(|c| &c.id == id)
            .map(|i| 2 + folders.len() + i)
            .unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Organization;

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

    fn collections() -> Vec<Collection> {
        vec![
            Collection {
                id: "c1".into(),
                name: "Engineering".into(),
                organization_id: Some("o1".into()),
            },
            Collection {
                id: "c2".into(),
                name: "Loose".into(),
                organization_id: None,
            },
        ]
    }

    fn organizations() -> Vec<Organization> {
        vec![Organization {
            id: "o1".into(),
            name: "Acme".into(),
        }]
    }

    #[test]
    fn all_matches_any_metadata() {
        let f = FolderFilter::All;
        assert!(f.matches(None, &[]));
        assert!(f.matches(Some("f1"), &[]));
        assert!(f.matches(Some("anything"), &["c1".into()]));
    }

    #[test]
    fn no_folder_matches_only_when_folder_is_none() {
        let f = FolderFilter::NoFolder;
        assert!(f.matches(None, &[]));
        assert!(!f.matches(Some("f1"), &[]));

        assert!(f.matches(None, &["c1".into()]));
    }

    #[test]
    fn specific_folder_matches_only_that_id() {
        let f = FolderFilter::Folder("f1".into());
        assert!(f.matches(Some("f1"), &[]));
        assert!(!f.matches(Some("f2"), &[]));
        assert!(!f.matches(None, &[]));
    }

    #[test]
    fn collection_filter_matches_when_collection_id_is_in_list() {
        let f = FolderFilter::Collection("c1".into());
        assert!(f.matches(None, &["c1".into()]));
        assert!(f.matches(Some("any"), &["c0".into(), "c1".into(), "c2".into()]));
        assert!(!f.matches(None, &[]));
        assert!(!f.matches(Some("any"), &["c2".into()]));
    }

    #[test]
    fn row_count_sums_meta_folders_and_collections() {
        assert_eq!(row_count(&[], &[]), 2);
        assert_eq!(row_count(&folders(), &[]), 4);
        assert_eq!(row_count(&[], &collections()), 4);
        assert_eq!(row_count(&folders(), &collections()), 6);
    }

    #[test]
    fn row_label_orders_meta_then_folders_then_collections() {
        let fs = folders();
        let cs = collections();
        let os = organizations();
        assert_eq!(row_label(0, &fs, &cs, &os).as_deref(), Some("All folders"));
        assert_eq!(row_label(1, &fs, &cs, &os).as_deref(), Some("(No folder)"));
        assert_eq!(row_label(2, &fs, &cs, &os).as_deref(), Some("Work"));
        assert_eq!(row_label(3, &fs, &cs, &os).as_deref(), Some("Personal"));

        assert_eq!(
            row_label(4, &fs, &cs, &os).as_deref(),
            Some("Acme / Engineering")
        );

        assert_eq!(row_label(5, &fs, &cs, &os).as_deref(), Some("Loose"));

        assert_eq!(row_label(6, &fs, &cs, &os), None);
    }

    #[test]
    fn filter_for_row_round_trips_back_via_row_for_filter() {
        let fs = folders();
        let cs = collections();
        for idx in 0..row_count(&fs, &cs) {
            let filter = filter_for_row(idx, &fs, &cs);
            assert_eq!(row_for_filter(&filter, &fs, &cs), idx);
        }
    }

    #[test]
    fn filter_for_row_returns_collection_for_collection_rows() {
        let fs = folders();
        let cs = collections();

        let f = filter_for_row(4, &fs, &cs);
        assert_eq!(f, FolderFilter::Collection("c1".into()));
        let f = filter_for_row(5, &fs, &cs);
        assert_eq!(f, FolderFilter::Collection("c2".into()));
    }

    #[test]
    fn filter_for_row_out_of_range_falls_back_to_all() {
        let fs = folders();
        let cs = collections();
        assert_eq!(filter_for_row(99, &fs, &cs), FolderFilter::All);
    }

    #[test]
    fn row_for_filter_unknown_id_falls_back_to_zero() {
        let fs = folders();
        let cs = collections();
        let unknown_folder = FolderFilter::Folder("missing-uuid".into());
        assert_eq!(row_for_filter(&unknown_folder, &fs, &cs), 0);
        let unknown_collection = FolderFilter::Collection("missing-uuid".into());
        assert_eq!(row_for_filter(&unknown_collection, &fs, &cs), 0);
    }
}
