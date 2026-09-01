use crossterm::event::{KeyCode, KeyEvent};

use crate::tui::app::App;
use crate::tui::flows::reprompt::{cancel as cancel_popup, verify_and_run};

pub fn handle(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => return cancel_popup(app),
        KeyCode::Enter => return verify_and_run(app),
        _ => {}
    }

    let Some(state) = app.reprompt.as_mut() else {
        return;
    };

    state.error = false;
    crate::tui::input::common::route_line_editor(&mut state.input, key);
}
