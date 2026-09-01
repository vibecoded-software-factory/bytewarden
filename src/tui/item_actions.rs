use crate::domain::item::Item;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemAction {
    Open,

    CopyUsername,

    CopyPassword,

    CopyTotp,

    Edit,

    Move,

    ToggleFavorite,

    Restore,

    Delete,
}

impl ItemAction {
    pub fn label(self) -> &'static str {
        match self {
            ItemAction::Open => "Open",
            ItemAction::CopyUsername => "Copy username",
            ItemAction::CopyPassword => "Copy password",
            ItemAction::CopyTotp => "Copy TOTP",
            ItemAction::Edit => "Edit",
            ItemAction::Move => "Move to collection",
            ItemAction::ToggleFavorite => "Toggle favorite",
            ItemAction::Restore => "Restore",
            ItemAction::Delete => "Delete",
        }
    }
}

pub struct ItemActionsState {
    pub item_id: String,

    pub actions: Vec<ItemAction>,

    pub cursor: usize,
}

pub fn actions_for(item: &Item, is_trash: bool, can_move: bool) -> Vec<ItemAction> {
    if is_trash {
        return vec![ItemAction::Open, ItemAction::Restore, ItemAction::Delete];
    }
    let mut v = vec![ItemAction::Open];
    if let Some(login) = &item.login {
        if login.username.as_deref().is_some_and(|s| !s.is_empty()) {
            v.push(ItemAction::CopyUsername);
        }
        if login.password.as_deref().is_some_and(|s| !s.is_empty()) {
            v.push(ItemAction::CopyPassword);
        }
        if login.totp.as_deref().is_some_and(|s| !s.is_empty()) {
            v.push(ItemAction::CopyTotp);
        }
    }
    v.push(ItemAction::Edit);
    if can_move {
        v.push(ItemAction::Move);
    }
    v.push(ItemAction::ToggleFavorite);
    v.push(ItemAction::Delete);
    v
}
