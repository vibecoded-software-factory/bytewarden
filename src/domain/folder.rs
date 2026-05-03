//! Vault folder type.
//!
//! Folders are flat (no nesting) organisational containers for items.
//! Each item carries an optional `folder_id` that points to one
//! folder; items without a folder_id appear in the implicit "no folder"
//! bucket. Deleting a folder leaves its items intact (their `folder_id`
//! becomes null).

use serde::Deserialize;

/// One vault folder. Mirrors the shape `bw list folders` returns, but
/// lives in the domain layer so any future adapter must produce the
/// same fields.
#[derive(Debug, Clone, Deserialize)]
pub struct Folder {
    /// Stable Bitwarden folder identifier (UUID).
    pub id: String,

    /// User-visible folder name.
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_from_bw_list_folders_shape() {
        let json = r#"[
            {"id":"f1","name":"Work"},
            {"id":"f2","name":"Personal"}
        ]"#;
        let folders: Vec<Folder> = serde_json::from_str(json).expect("parse");
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].id, "f1");
        assert_eq!(folders[1].name, "Personal");
    }

    #[test]
    fn deserialize_empty_folder_list() {
        let folders: Vec<Folder> = serde_json::from_str("[]").expect("parse");
        assert!(folders.is_empty());
    }
}
