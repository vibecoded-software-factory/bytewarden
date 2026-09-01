use crate::domain::LineEditor;

#[derive(Debug, Clone)]
pub enum ProtectedAction {
    CopyPassword,

    CopyTotp(String),

    CopySelectedDetailField,

    RevealDetail,

    RevealEditField,
}

#[derive(Debug)]
pub struct RepromptState {
    pub input: LineEditor,

    pub after: ProtectedAction,

    pub error: bool,

    pub origin: crate::tui::screens::Screen,
}

impl RepromptState {
    pub fn new(after: ProtectedAction, origin: crate::tui::screens::Screen) -> Self {
        Self {
            input: LineEditor::new(),
            after,
            error: false,
            origin,
        }
    }
}
