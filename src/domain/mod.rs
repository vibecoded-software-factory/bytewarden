//! Pure domain layer.
//!
//! This module contains the core entities of the password manager — vault
//! items, filters, status enums, search ranking, identity helpers — and
//! nothing else. It has no knowledge of:
//!
//! * the Bitwarden CLI,
//! * the filesystem or any configuration format,
//! * the terminal, [`ratatui`](https://docs.rs/ratatui), or any rendering API.
//!
//! Because the layer is I/O-free it can be unit-tested without spawning
//! processes or mocking files. Every other layer in the crate ([`crate::ports`],
//! [`crate::adapters`], [`crate::tui`]) depends on this one and never the
//! other way around.

pub mod filter;
pub mod folder;
pub mod identity;
pub mod item;
pub mod line_editor;
pub mod membership;
pub mod search;
pub mod validation;
pub mod vault_info;

pub use filter::{CREATE_ITEM_TYPES, CreateItemType, ITEM_FILTERS, ItemFilter};
pub use folder::Folder;
pub use identity::{build_full_name, identity_fields};
pub use item::{
    Attachment, CardData, Field, IdentityData, Item, LoginData, SshKeyData, UriData, UriMatch,
    item_type_label,
};
pub use line_editor::LineEditor;
pub use membership::{Collection, Organization};
pub use search::{LoweredItem, fuzzy_score, fuzzy_score_lowered};
pub use vault_info::{LoginOutcome, TwoFactorMethod, VaultInfo, VaultStatus};
