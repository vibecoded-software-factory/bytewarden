use crate::domain::filter::CreateItemType;
use crate::tui::edit_field::EditField;

#[derive(Default)]
pub struct EditForm {
    pub fields: Vec<EditField>,

    pub field_idx: usize,

    pub item_id: String,

    pub active: bool,
}

impl EditForm {
    pub fn field_mut(&mut self) -> Option<&mut EditField> {
        self.fields.get_mut(self.field_idx)
    }

    pub fn toggle_reveal(&mut self) {
        if let Some(f) = self.field_mut()
            && f.hidden
        {
            f.revealed = !f.revealed;
        }
    }
}

pub struct CreateForm {
    pub fields: Vec<EditField>,

    pub field_idx: usize,

    pub item_type: CreateItemType,

    pub type_idx: usize,

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
    pub fn field_mut(&mut self) -> Option<&mut EditField> {
        self.fields.get_mut(self.field_idx)
    }
}
