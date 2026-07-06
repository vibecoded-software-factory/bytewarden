//! Ratatui rendering — one module per top-level screen plus shared
//! helpers under [`widgets`], [`action`], [`starfield`], [`logo`].

pub mod action;
pub mod assign_collections;
pub mod attachment_download;
pub mod attachment_upload;
pub mod confirm;
pub mod confirm_delete_attachment;
pub mod create;
pub mod detail;
pub mod export;
pub mod folder_delete_confirm;
pub mod folder_name;
pub mod generator;
pub mod help;
pub mod import;
pub mod login;
pub mod logo;
pub mod logout_confirm;
pub mod memberships;
pub mod palette;
pub mod rename_field;
pub mod reprompt;
pub mod send_create;
pub mod settings;
pub mod splash;
pub mod starfield;
pub mod vault;
pub mod widgets;

use ratatui::{
    Frame,
    layout::Alignment,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::tui::app::App;
use crate::tui::screens::Screen;
use crate::tui::theme::Theme;

/// Smallest terminal in which the regular layouts render without
/// overlapping or clipping critical UI. Below this we show a polite
/// "resize me" message instead — every screen's layout has at least
/// one place where assumptions about minimum size show up (login form
/// width, vault sidebar, help popup), and trying to render through
/// them in a 5×10 terminal produces nonsense rather than a crash.
const MIN_TERM_WIDTH: u16 = 60;
const MIN_TERM_HEIGHT: u16 = 18;

/// Pure predicate so the size check is testable without spinning up a
/// real terminal. Either dimension below the floor counts as "too
/// small".
pub fn is_terminal_too_small(width: u16, height: u16) -> bool {
    width < MIN_TERM_WIDTH || height < MIN_TERM_HEIGHT
}

/// Renders a centred "terminal too small" message — the only thing we
/// dare draw when the area is below [`MIN_TERM_WIDTH`] /
/// [`MIN_TERM_HEIGHT`]. Picks the error color from the theme so it
/// reads correctly under both light and dark palettes.
fn draw_too_small(frame: &mut Frame, theme: &Theme) {
    let area = frame.area();
    let header = Line::from(Span::styled(
        "Terminal too small",
        Style::default()
            .fg(theme.error)
            .add_modifier(Modifier::BOLD),
    ))
    .alignment(Alignment::Center);
    let detail = Line::from(Span::styled(
        format!(
            "Resize to at least {}×{} (currently {}×{})",
            MIN_TERM_WIDTH, MIN_TERM_HEIGHT, area.width, area.height,
        ),
        Style::default().fg(theme.dim),
    ))
    .alignment(Alignment::Center);
    let hint = Line::from(Span::styled(
        "Ctrl+C to quit",
        Style::default().fg(theme.dim),
    ))
    .alignment(Alignment::Center);
    // Vertically center: pad the top with empty lines so the header
    // lands roughly mid-screen even on 5-row terminals.
    let blanks = (area.height as usize).saturating_sub(3) / 2;
    let mut lines: Vec<Line> = (0..blanks).map(|_| Line::from("")).collect();
    lines.push(header);
    lines.push(detail);
    lines.push(hint);
    frame.render_widget(Paragraph::new(lines), area);
}

/// Top-level frame router — invoked once per terminal redraw.
pub fn draw(frame: &mut Frame, app: &mut App) {
    // Clear the scroll registry each frame; every scrollable surface
    // re-registers its region as it draws, so the wheel dispatches by position.
    crate::tui::view::widgets::reset_scroll_regions();
    if is_terminal_too_small(frame.area().width, frame.area().height) {
        draw_too_small(frame, &app.theme);
        return;
    }
    match app.screen {
        Screen::Splash => splash::draw(frame, app),
        Screen::Login => login::draw(frame, app),
        Screen::Vault => vault::draw(frame, app),
        Screen::Detail => detail::draw(frame, app),
        Screen::Help => {
            // Draw the screen the user opened help from underneath, so
            // the popup feels overlaid on the right context (Login,
            // Detail, etc.) — not always the vault.
            match app.help_from.clone().unwrap_or(Screen::Vault) {
                Screen::Login => login::draw(frame, app),
                Screen::Detail => detail::draw(frame, app),
                Screen::Create => create::draw(frame, app),
                _ => vault::draw(frame, app),
            }
            help::draw_popup(frame, frame.area(), app);
        }
        Screen::Settings => {
            // Draw the originating screen underneath so the overlay feels
            // in context, then the Settings popup on top.
            match app.settings_ui.from.clone() {
                Screen::Login => login::draw(frame, app),
                Screen::Detail => detail::draw(frame, app),
                Screen::Create => create::draw(frame, app),
                Screen::Generator => generator::draw(frame, app),
                _ => vault::draw(frame, app),
            }
            settings::draw_popup(frame, frame.area(), app);
        }
        Screen::Create => create::draw(frame, app),
        Screen::ConfirmDelete => {
            vault::draw(frame, app);
            confirm::draw_popup(frame, frame.area(), app);
        }
        Screen::ConfirmLogout => {
            vault::draw(frame, app);
            logout_confirm::draw_popup(frame, frame.area(), app);
        }
        Screen::Generator => generator::draw(frame, app),
        Screen::RenameField => {
            // Draw the underlying detail+edit-mode screen first so the
            // popup feels overlaid in context.
            detail::draw(frame, app);
            rename_field::draw_popup(frame, frame.area(), app);
        }
        Screen::FolderName => {
            vault::draw(frame, app);
            folder_name::draw_popup(frame, frame.area(), app);
        }
        Screen::ConfirmDeleteFolder => {
            vault::draw(frame, app);
            folder_delete_confirm::draw_popup(frame, frame.area(), app);
        }
        Screen::Export => {
            vault::draw(frame, app);
            export::draw_popup(frame, frame.area(), app);
        }
        Screen::Import => {
            vault::draw(frame, app);
            import::draw_popup(frame, frame.area(), app);
        }
        Screen::AttachmentUpload => {
            detail::draw(frame, app);
            attachment_upload::draw_popup(frame, frame.area(), app);
        }
        Screen::AttachmentDownload => {
            detail::draw(frame, app);
            attachment_download::draw_popup(frame, frame.area(), app);
        }
        Screen::ConfirmDeleteAttachment => {
            detail::draw(frame, app);
            confirm_delete_attachment::draw_popup(frame, frame.area(), app);
        }
        Screen::SendCreate => {
            vault::draw(frame, app);
            send_create::draw_popup(frame, frame.area(), app);
        }
        Screen::Memberships => {
            vault::draw(frame, app);
            memberships::draw_popup(frame, frame.area(), app);
        }
        Screen::RepromptUnlock => {
            // Draw the underlying screen the user came from so the
            // popup feels overlaid in the right context. The state
            // captures the origin at open time so we don't have to
            // guess from heuristics.
            let origin = app
                .reprompt
                .as_ref()
                .map(|s| s.origin.clone())
                .unwrap_or(Screen::Vault);
            match origin {
                Screen::Detail => detail::draw(frame, app),
                _ => vault::draw(frame, app),
            }
            reprompt::draw_popup(frame, frame.area(), app);
        }
        Screen::AssignCollections => {
            // Opened from either edit-mode (detail screen) or the
            // create form. The state captures the origin so we draw
            // the right context underneath the popup.
            let origin = app
                .assign_collections
                .as_ref()
                .map(|s| s.origin.clone())
                .unwrap_or(Screen::Detail);
            match origin {
                Screen::Create => create::draw(frame, app),
                _ => detail::draw(frame, app),
            }
            assign_collections::draw_popup(frame, frame.area(), app);
        }
        Screen::CommandPalette => {
            // Opened from the vault or the detail screen (its state
            // records which), drawn underneath the centered modal.
            let origin = app
                .palette
                .as_ref()
                .map(|s| s.origin.clone())
                .unwrap_or(Screen::Vault);
            match origin {
                Screen::Detail => detail::draw(frame, app),
                _ => vault::draw(frame, app),
            }
            palette::draw(frame, app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typical_terminal_is_not_too_small() {
        // 80×24 is the classic VT100 default — must always render.
        assert!(!is_terminal_too_small(80, 24));
        // Comfortable modern default.
        assert!(!is_terminal_too_small(120, 40));
        // Right at the threshold.
        assert!(!is_terminal_too_small(MIN_TERM_WIDTH, MIN_TERM_HEIGHT));
    }

    #[test]
    fn tiny_terminal_is_too_small() {
        assert!(is_terminal_too_small(5, 10));
        assert!(is_terminal_too_small(0, 0));
    }

    #[test]
    fn either_dimension_below_floor_triggers_fallback() {
        assert!(is_terminal_too_small(MIN_TERM_WIDTH - 1, 50));
        assert!(is_terminal_too_small(200, MIN_TERM_HEIGHT - 1));
    }
}
