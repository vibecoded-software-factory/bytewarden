#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Screen {
    Splash,

    Login,

    Vault,

    Detail,

    Help,

    Settings,

    Create,

    ConfirmDelete,

    ConfirmLogout,

    Generator,

    RenameField,

    FolderName,

    ConfirmDeleteFolder,

    Export,

    Import,

    AttachmentUpload,

    AttachmentDownload,

    ConfirmDeleteAttachment,

    SendCreate,

    Memberships,

    RepromptUnlock,

    AssignCollections,

    CommandPalette,

    ItemActions,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Focus {
    Status,

    Search,

    Folders,

    Items,

    List,

    CmdLog,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum LoginField {
    Server,
    Email,
    Password,

    Otp,

    SaveEmail,

    AutoLock,

    KeepSession,
}
