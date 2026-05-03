//! Login / unlock screen renderer.

use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Padding, Paragraph},
};

use crate::tui::app::App;
use crate::tui::screens::LoginField;
use crate::tui::view::action::action_line;
use crate::tui::view::logo;
use crate::tui::view::starfield::fill_stars;
use crate::tui::view::widgets::{
    focus_border, input_with_cursor, render_checkbox, render_cmd_bar_with_help, rounded_block,
};

/// Renders the login screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    let t = &app.theme;
    let area = frame.area();

    // Form rows: padding(1)+server-lbl(1)+server-in(3)
    //            +email-lbl(1)+email-in(3)+pass-lbl(1)+pass-in(3)
    //            + [otp-lbl(1)+otp-in(3)]
    //            +save(1)+lock(1)+keep_session(1)+strip(2)+border(2).
    let form_height: u16 = if app.otp_required { 24 } else { 20 };

    // Vertical layout — stars above the form (2/3) and below (1/3),
    // command bar at the bottom.
    let c = Layout::vertical([
        Constraint::Fill(2),
        Constraint::Length(form_height),
        Constraint::Fill(1),
        Constraint::Length(2),
    ])
    .split(area);
    let (logo_chunk, form_chunk, lower_chunk, bar_chunk) = (c[0], c[1], c[2], c[3]);

    if logo_chunk.height >= 6 {
        logo::render(frame, app, logo_chunk);
    } else {
        fill_stars(frame, logo_chunk, t);
    }
    fill_stars(frame, lower_chunk, t);

    // Center the form and pick its border color from error state.
    let form_w = area.width.saturating_sub(8).clamp(44, 72);
    let form_row = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(form_w),
        Constraint::Fill(1),
    ])
    .split(form_chunk);
    // Fill the gutters either side of the form with starfield so the
    // splash background is continuous across the whole screen — without
    // the form panel itself losing readability.
    fill_stars(frame, form_row[0], t);
    fill_stars(frame, form_row[2], t);
    let form_area = form_row[1];
    let form_border = if app.login_error {
        Style::default().fg(t.error)
    } else {
        Style::default().fg(t.accent)
    };
    let block = Block::default()
        .title(" Login ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(form_border)
        .padding(Padding::horizontal(2));
    let inner = block.inner(form_area);
    frame.render_widget(block, form_area);
    app.mouse_areas.login = Some(form_area);

    // Build the inner vertical splits dynamically — OTP rows only exist
    // when needed.
    let (idx_otp_lbl, idx_otp_in, idx_save, idx_lock, idx_keep, idx_strip, f);
    if app.otp_required {
        let splits = Layout::vertical([
            Constraint::Length(1), // [0]  padding
            Constraint::Length(1), // [1]  server label
            Constraint::Length(3), // [2]  server input
            Constraint::Length(1), // [3]  email label
            Constraint::Length(3), // [4]  email input
            Constraint::Length(1), // [5]  pass label
            Constraint::Length(3), // [6]  pass input
            Constraint::Length(1), // [7]  otp label
            Constraint::Length(3), // [8]  otp input
            Constraint::Length(1), // [9]  save email
            Constraint::Length(1), // [10] auto-lock
            Constraint::Length(1), // [11] keep session
            Constraint::Length(2), // [12] feedback strip
        ])
        .split(inner);
        (
            idx_otp_lbl,
            idx_otp_in,
            idx_save,
            idx_lock,
            idx_keep,
            idx_strip,
        ) = (7, 8, 9, 10, 11, 12);
        f = splits;
    } else {
        let splits = Layout::vertical([
            Constraint::Length(1), // [0]  padding
            Constraint::Length(1), // [1]  server label
            Constraint::Length(3), // [2]  server input
            Constraint::Length(1), // [3]  email label
            Constraint::Length(3), // [4]  email input
            Constraint::Length(1), // [5]  pass label
            Constraint::Length(3), // [6]  pass input
            Constraint::Length(1), // [7]  save email
            Constraint::Length(1), // [8]  auto-lock
            Constraint::Length(1), // [9]  keep session
            Constraint::Length(2), // [10] feedback strip
        ])
        .split(inner);
        (
            idx_otp_lbl,
            idx_otp_in,
            idx_save,
            idx_lock,
            idx_keep,
            idx_strip,
        ) = (0, 0, 7, 8, 9, 10);
        f = splits;
    }
    let _ = (idx_otp_lbl, idx_otp_in); // silence unused-variable in non-OTP branch

    // ── Server ────────────────────────────────────────────────────────────
    let server_dirty = app.server_input.trim() != app.server_committed;
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Server:", Style::default().fg(t.dim)),
            Span::styled(
                if server_dirty {
                    "  (Enter or Tab to apply)"
                } else {
                    ""
                },
                Style::default().fg(if server_dirty { t.accent } else { t.dim }),
            ),
        ])),
        f[1],
    );
    let server_foc = app.active_field == LoginField::Server;
    frame.render_widget(
        Paragraph::new(input_with_cursor(
            &app.server_input,
            app.server_cursor,
            server_foc,
            t,
        ))
        .block(rounded_block(focus_border(server_foc, t.accent))),
        f[2],
    );

    // ── Email ─────────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new("Email:").style(Style::default().fg(t.dim)),
        f[3],
    );
    let email_foc = app.active_field == LoginField::Email;
    frame.render_widget(
        Paragraph::new(input_with_cursor(
            &app.email_input,
            app.email_cursor,
            email_foc,
            t,
        ))
        .block(rounded_block(focus_border(email_foc, t.accent))),
        f[4],
    );

    // ── Password ──────────────────────────────────────────────────────────
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Master Password:", Style::default().fg(t.dim)),
            Span::styled(
                "  (F2: reveal)",
                Style::default().fg(if app.login_password_visible {
                    t.accent
                } else {
                    t.dim
                }),
            ),
        ])),
        f[5],
    );
    let pass_foc = app.active_field == LoginField::Password;
    let pass_line = if app.login_password_visible {
        input_with_cursor(&app.password_input, app.password_cursor, pass_foc, t)
    } else {
        let masked_before = "●".repeat(app.password_cursor);
        let masked_after = "●".repeat(
            app.password_input
                .chars()
                .count()
                .saturating_sub(app.password_cursor),
        );
        if pass_foc {
            Line::from(vec![
                Span::raw(masked_before),
                Span::styled("█", Style::default().fg(t.accent)),
                Span::raw(masked_after),
            ])
        } else {
            Line::from(Span::raw("●".repeat(app.password_input.chars().count())))
        }
    };
    frame.render_widget(
        Paragraph::new(pass_line).block(rounded_block(focus_border(pass_foc, t.accent))),
        f[6],
    );

    // ── OTP ──────────────────────────────────────────────────────────────
    if app.otp_required {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("Verification Code:", Style::default().fg(t.dim)),
                Span::styled("  (sent to your email)", Style::default().fg(t.dim)),
            ])),
            f[idx_otp_lbl],
        );
        let otp_foc = app.active_field == LoginField::Otp;
        frame.render_widget(
            Paragraph::new(input_with_cursor(
                &app.otp_input,
                app.otp_cursor,
                otp_foc,
                t,
            ))
            .block(rounded_block(focus_border(otp_foc, t.accent))),
            f[idx_otp_in],
        );
    }

    // ── Checkboxes ────────────────────────────────────────────────────────
    render_checkbox(
        frame,
        "Save email",
        app.save_email,
        app.active_field == LoginField::SaveEmail,
        t.accent,
        t.inactive,
        f[idx_save],
    );
    let lock_label = format!("Auto-lock after {} min", app.lock_after_secs / 60);
    render_checkbox(
        frame,
        &lock_label,
        app.auto_lock,
        app.active_field == LoginField::AutoLock,
        t.accent,
        t.inactive,
        f[idx_lock],
    );
    render_checkbox(
        frame,
        "Keep session",
        app.keep_session,
        app.active_field == LoginField::KeepSession,
        t.accent,
        t.inactive,
        f[idx_keep],
    );

    // ── Feedback strip ────────────────────────────────────────────────────
    let strip_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(t.muted));
    if app.login_error {
        let msg = if app.otp_required {
            "Invalid verification code. Please try again."
        } else {
            "Invalid credentials. Please try again."
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " ✕ ",
                    Style::default().fg(t.error).add_modifier(Modifier::BOLD),
                ),
                Span::styled(msg, Style::default().fg(t.error)),
            ]))
            .block(strip_block),
            f[idx_strip],
        );
    } else if app.otp_required {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " ✉ ",
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Check your email for the verification code.",
                    Style::default().fg(t.accent),
                ),
            ]))
            .block(strip_block),
            f[idx_strip],
        );
    } else if let Some(line) = action_line(app) {
        frame.render_widget(Paragraph::new(line).block(strip_block), f[idx_strip]);
    }

    // ── Bottom hints bar ──────────────────────────────────────────────────
    render_cmd_bar_with_help(
        frame,
        area,
        bar_chunk,
        "Tab: field  |  Enter: login  |  F2: reveal pwd",
        "Tab  Enter  F2",
        t.dim,
        t,
    );
}
