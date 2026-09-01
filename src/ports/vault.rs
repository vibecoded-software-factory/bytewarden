use super::BwError;
use crate::domain::{
    Collection, Folder, Item, LoginOutcome, Organization, TwoFactorMethod, VaultInfo,
};

pub struct ParallelSessionData {
    pub folders: Result<Vec<Folder>, BwError>,
    pub organizations: Result<Vec<Organization>, BwError>,
    pub collections: Result<Vec<Collection>, BwError>,
    pub import_formats: Result<Vec<String>, BwError>,
}

pub trait VaultPort {
    fn status(&mut self) -> Result<VaultInfo, BwError>;

    fn login(&mut self, email: &str, password: &str) -> LoginOutcome;

    fn login_with_otp(&mut self, email: &str, password: &str, otp: &str)
    -> Result<String, BwError>;

    fn login_with_two_factor(
        &mut self,
        email: &str,
        password: &str,
        code: &str,
        method: TwoFactorMethod,
    ) -> Result<String, BwError>;

    fn login_with_api_key(&mut self) -> Result<(), BwError>;

    fn login_with_sso(&mut self) -> Result<(), BwError>;

    fn unlock(&mut self, password: &str) -> Result<String, BwError>;

    fn lock(&mut self);

    fn logout(&mut self) -> Result<(), BwError>;

    fn session_key(&self) -> Option<&str>;

    fn set_server(&mut self, url: &str) -> Result<(), BwError>;

    fn list_items(&mut self) -> Result<Vec<Item>, BwError>;

    fn list_trash(&mut self) -> Result<Vec<Item>, BwError>;

    fn sync(&mut self) -> Result<(), BwError>;

    fn get_totp(&mut self, item_id: &str) -> Result<String, BwError>;

    fn get_item_json(&mut self, item_id: &str) -> Result<zeroize::Zeroizing<String>, BwError>;

    fn check_exposed(&mut self, item_id: &str) -> Result<u32, BwError>;

    fn create_item(&mut self, item_json: &str) -> Result<Item, BwError>;

    fn edit_item(&mut self, item_id: &str, item_json: &str) -> Result<Item, BwError>;

    fn delete_item(&mut self, item_id: &str, permanent: bool) -> Result<(), BwError>;

    fn restore_item(&mut self, item_id: &str) -> Result<(), BwError>;

    fn list_folders(&mut self) -> Result<Vec<Folder>, BwError>;

    fn create_folder(&mut self, name: &str) -> Result<Folder, BwError>;

    fn edit_folder(&mut self, folder_id: &str, name: &str) -> Result<Folder, BwError>;

    fn delete_folder(&mut self, folder_id: &str) -> Result<(), BwError>;

    fn export(&mut self, format: &str, output_path: &str) -> Result<(), BwError>;

    fn get_fingerprint(&mut self) -> Result<String, BwError>;

    fn import(&mut self, format: &str, input_path: &str) -> Result<(), BwError>;

    fn list_import_formats(&mut self) -> Result<Vec<String>, BwError>;

    fn move_item(
        &mut self,
        item_id: &str,
        organization_id: &str,
        collection_ids: &[String],
    ) -> Result<(), BwError>;

    fn upload_attachment(&mut self, item_id: &str, file_path: &str) -> Result<Item, BwError>;

    fn download_attachment(
        &mut self,
        item_id: &str,
        file_name: &str,
        output_path: &str,
    ) -> Result<(), BwError>;

    fn delete_attachment(&mut self, item_id: &str, attachment_id: &str) -> Result<(), BwError>;

    fn send_text(
        &mut self,
        name: &str,
        days_to_expire: u8,
        content: &str,
    ) -> Result<String, BwError>;

    fn list_organizations(&mut self) -> Result<Vec<Organization>, BwError>;

    fn list_collections(&mut self) -> Result<Vec<Collection>, BwError>;

    fn parallel_session_data(&mut self) -> ParallelSessionData {
        ParallelSessionData {
            folders: self.list_folders(),
            organizations: self.list_organizations(),
            collections: self.list_collections(),
            import_formats: self.list_import_formats(),
        }
    }
}
