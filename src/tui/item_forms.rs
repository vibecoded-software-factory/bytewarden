//! Edit- and create-item form state.
//!
//! The two item forms split out of the [`crate::tui::app::App`]
//! god-struct into their own screen-local containers, following the
//! same pattern the login form and the popup states already use. Both
//! forms are built as ordered [`EditField`] rows by the shared builders
//! in [`crate::tui::edit_field`]; these structs just hold the live
//! buffer + cursor + the small amount of per-form mode state.

use crate::domain::filter::CreateItemType;
use crate::tui::edit_field::EditField;

/// The edit-item form — the Detail screen's editable mode.
#[derive(Default)]
pub struct EditForm {
    /// The editable rows for the item being edited.
    pub fields: Vec<EditField>,
    /// Cursor into [`Self::fields`].
    pub field_idx: usize,
    /// Id of the item currently being edited (used to route the save).
    pub item_id: String,
    /// Whether the Detail screen is currently showing the editable form.
    pub active: bool,
}

impl EditForm {
    /// The focused editable field, if any.
    pub fn field_mut(&mut self) -> Option<&mut EditField> {
        self.fields.get_mut(self.field_idx)
    }

    /// Toggles the reveal flag on the focused (hidden) field.
    pub fn toggle_reveal(&mut self) {
        if let Some(f) = self.field_mut()
            && f.hidden
        {
            f.revealed = !f.revealed;
        }
    }
}

/// The create-item form.
pub struct CreateForm {
    /// The (initially empty) rows for the new item.
    pub fields: Vec<EditField>,
    /// Cursor into [`Self::fields`].
    pub field_idx: usize,
    /// Which item type is being created.
    pub item_type: CreateItemType,
    /// Highlighted type in the type-picker step.
    pub type_idx: usize,
    /// `true` while the user is still on the type-picker step (before
    /// the field form is shown).
    pub choosing_type: bool,
}

impl Default for CreateForm {
    fn default() -> Self {
        Self {
            fields: Vec::new(),
            field_idx: 0,
            item_type: CreateItemType::Login,
            type_idx: 0,
            choosing_type: true,
        }
    }
}

impl CreateForm {
    /// The focused editable field, if any.
    pub fn field_mut(&mut self) -> Option<&mut EditField> {
        self.fields.get_mut(self.field_idx)
    }
}
