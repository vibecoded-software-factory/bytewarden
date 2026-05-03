//! Vault list filters and "create item" type selector.

use crate::domain::item::{
    ITEM_TYPE_CARD, ITEM_TYPE_IDENTITY, ITEM_TYPE_LOGIN, ITEM_TYPE_SECURE_NOTE, ITEM_TYPE_SSH_KEY,
    Item,
};

/// Categorical filter applied to the vault list.
///
/// `Trash` is special: items are not included in the regular vault listing,
/// they live in a separate trash area fetched on demand.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ItemFilter {
    /// All non-trashed items, regardless of type.
    All,
    /// Items where `favorite == true`.
    Favorites,
    /// Logins (`item_type == 1`).
    Login,
    /// Cards (`item_type == 3`).
    Card,
    /// Identities (`item_type == 4`).
    Identity,
    /// Secure notes (`item_type == 2`).
    SecureNote,
    /// SSH keys (`item_type == 5`).
    SshKey,
    /// Trashed items — fetched separately via the vault port.
    Trash,
}

impl ItemFilter {
    /// Returns the human-readable label for the filter.
    pub fn label(&self) -> &'static str {
        match self {
            ItemFilter::All => "All Items",
            ItemFilter::Favorites => "Favorites",
            ItemFilter::Login => "Login",
            ItemFilter::Card => "Card",
            ItemFilter::Identity => "Identity",
            ItemFilter::SecureNote => "Secure Note",
            ItemFilter::SshKey => "SSH Key",
            ItemFilter::Trash => "Trash",
        }
    }

    /// Returns the underlying [`Item::item_type`] discriminant if this filter
    /// targets a single type, or `None` for the meta-filters
    /// ([`Self::All`], [`Self::Favorites`], [`Self::Trash`]).
    pub fn type_id(&self) -> Option<u8> {
        match self {
            ItemFilter::Login => Some(ITEM_TYPE_LOGIN),
            ItemFilter::SecureNote => Some(ITEM_TYPE_SECURE_NOTE),
            ItemFilter::Card => Some(ITEM_TYPE_CARD),
            ItemFilter::Identity => Some(ITEM_TYPE_IDENTITY),
            ItemFilter::SshKey => Some(ITEM_TYPE_SSH_KEY),
            _ => None,
        }
    }

    /// Returns `true` if the given item belongs to this filter.
    ///
    /// [`Self::Trash`] always returns `false` because trashed items are
    /// fetched separately and never present in the in-memory vault list.
    pub fn matches(&self, item: &Item) -> bool {
        match self {
            ItemFilter::All => true,
            ItemFilter::Favorites => item.favorite,
            ItemFilter::Trash => false,
            other => other.type_id() == Some(item.item_type),
        }
    }
}

/// Display order for the filter sidebar.
pub const ITEM_FILTERS: &[ItemFilter] = &[
    ItemFilter::All,
    ItemFilter::Favorites,
    ItemFilter::Login,
    ItemFilter::Card,
    ItemFilter::Identity,
    ItemFilter::SecureNote,
    ItemFilter::SshKey,
    ItemFilter::Trash,
];

/// Item types the user can create from the TUI.
///
/// Currently covers all of the Bitwarden item types except attachments
/// (which are handled separately on existing items).
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CreateItemType {
    Login,
    SecureNote,
    Card,
    Identity,
    SshKey,
}

impl CreateItemType {
    /// Human-readable label shown in the type-picker.
    pub fn label(&self) -> &'static str {
        match self {
            CreateItemType::Login => "Login",
            CreateItemType::SecureNote => "Secure Note",
            CreateItemType::Card => "Card",
            CreateItemType::Identity => "Identity",
            CreateItemType::SshKey => "SSH Key",
        }
    }
}

/// Display order for the "create item" type-picker.
pub const CREATE_ITEM_TYPES: &[CreateItemType] = &[
    CreateItemType::Login,
    CreateItemType::SecureNote,
    CreateItemType::Card,
    CreateItemType::Identity,
    CreateItemType::SshKey,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn item(item_type: u8, favorite: bool) -> Item {
        Item {
            id: "id".into(),
            name: "n".into(),
            item_type,
            login: None,
            card: None,
            identity: None,
            ssh_key: None,
            notes: None,
            folder_id: None,
            favorite,
            fields: vec![],
            attachments: None,
        }
    }

    #[test]
    fn type_id_is_some_for_concrete_types() {
        assert_eq!(ItemFilter::Login.type_id(), Some(ITEM_TYPE_LOGIN));
        assert_eq!(ItemFilter::Card.type_id(), Some(ITEM_TYPE_CARD));
        assert_eq!(ItemFilter::Identity.type_id(), Some(ITEM_TYPE_IDENTITY));
        assert_eq!(
            ItemFilter::SecureNote.type_id(),
            Some(ITEM_TYPE_SECURE_NOTE)
        );
        assert_eq!(ItemFilter::SshKey.type_id(), Some(ITEM_TYPE_SSH_KEY));
    }

    #[test]
    fn type_id_is_none_for_meta_filters() {
        assert_eq!(ItemFilter::All.type_id(), None);
        assert_eq!(ItemFilter::Favorites.type_id(), None);
        assert_eq!(ItemFilter::Trash.type_id(), None);
    }

    #[test]
    fn all_matches_every_item() {
        for ty in [1u8, 2, 3, 4, 5, 99] {
            assert!(ItemFilter::All.matches(&item(ty, false)));
        }
    }

    #[test]
    fn favorites_matches_only_starred() {
        assert!(ItemFilter::Favorites.matches(&item(1, true)));
        assert!(!ItemFilter::Favorites.matches(&item(1, false)));
    }

    #[test]
    fn trash_never_matches_in_memory_items() {
        // Trash is fetched separately — the in-memory list filter
        // should never accept anything as "trash".
        assert!(!ItemFilter::Trash.matches(&item(1, true)));
    }

    #[test]
    fn type_specific_filter_matches_only_its_type() {
        assert!(ItemFilter::Login.matches(&item(1, false)));
        assert!(!ItemFilter::Login.matches(&item(3, false)));
        assert!(ItemFilter::Card.matches(&item(3, false)));
        assert!(!ItemFilter::Card.matches(&item(1, false)));
    }

    #[test]
    fn item_filters_constant_covers_every_variant_in_order() {
        assert_eq!(ITEM_FILTERS.len(), 8);
        assert_eq!(ITEM_FILTERS[0], ItemFilter::All);
        assert_eq!(ITEM_FILTERS[7], ItemFilter::Trash);
    }

    #[test]
    fn create_types_have_labels() {
        for ct in CREATE_ITEM_TYPES {
            assert!(!ct.label().is_empty());
        }
        assert_eq!(CreateItemType::SshKey.label(), "SSH Key");
        assert_eq!(CreateItemType::SecureNote.label(), "Secure Note");
    }

    #[test]
    fn item_filter_labels_unique() {
        use std::collections::HashSet;
        let labels: HashSet<&'static str> = ITEM_FILTERS.iter().map(|f| f.label()).collect();
        assert_eq!(labels.len(), ITEM_FILTERS.len());
    }
}
