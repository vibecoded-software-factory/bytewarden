#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActionState {
    Idle,

    Running(String),

    Done(String),

    Error(String),
}

#[derive(Debug, Clone)]
pub struct CmdEntry {
    pub cmd: String,

    pub ok: bool,

    pub detail: String,
}
