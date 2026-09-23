use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Folder {
    pub id: String,

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
