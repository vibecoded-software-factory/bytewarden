use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
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
    editor_spans, editor_spans_masked, focus_border, render_checkbox, render_cmd_bar_with_help,
    rounded_block,
};

thread_local! {

    static LOGIN_HITS: std::cell::RefCell<Vec<(Rect, LoginField)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn register_field(rect: Rect, field: LoginField) {
    if rect.width > 0 && rect.height > 0 {
        LOGIN_HITS.with(|h| h.borrow_mut().push((rect, field)));
    }
}

pub fn login_field_at(column: u16, row: u16) -> Option<LoginField> {
    LOGIN_HITS.with(|h| {
        h.borrow()
            .iter()
            .rev()
            .find(|(r, _)| {
                column >= r.x && column < r.x + r.width && row >= r.y && row < r.y + r.height
            })
            .map(|(_, f)| f.clone())
    })
}

fn union(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let right = (a.x + a.width).max(b.x + b.width);
    let bottom = (a.y + a.height).max(b.y + b.height);
    Rect::new(x, y, right - x, bottom - y)
}

pub fn draw(frame: &mut Frame, app: &mut App) {
    if matches!(
        app.action_state,
        crate::tui::action::ActionState::Running(_)
    ) {
        crate::tui::view::splash::draw(frame, app);
        return;
    }

    LOGIN_HITS.with(|h| h.borrow_mut().clear());
    let t = &app.theme;
    let area = frame.area();

    let form_height: u16 = if app.login.awaiting_code() { 24 } else { 20 };

    let c = Layout::vertical([
        Constraint::Fill(2),
        Constraint::Length(form_height),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .split(area);
    let (logo_chunk, form_chunk, lower_chunk, bar_chunk) = (c[0], c[1], c[2], c[3]);

    if logo_chunk.height >= 6 {
        logo::render(frame, app, logo_chunk);
    } else {
        fill_stars(frame, logo_chunk, t);
    }
    fill_stars(frame, lower_chunk, t);

    let form_w = area.width.saturating_sub(8).clamp(44, 72);
    let form_row = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Length(form_w),
        Constraint::Fill(1),
    ])
    .split(form_chunk);

    fill_stars(frame, form_row[0], t);
    fill_stars(frame, form_row[2], t);
    let form_area = form_row[1];
    let form_border = if app.login.login_error {
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

    let (idx_otp_lbl, idx_otp_in, idx_save, idx_lock, idx_keep, idx_strip, f);
    if app.login.awaiting_code() {
        let splits = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
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
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(2),
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

    register_field(union(f[1], f[2]), LoginField::Server);
    register_field(union(f[3], f[4]), LoginField::Email);
    register_field(union(f[5], f[6]), LoginField::Password);
    if app.login.awaiting_code() {
        register_field(union(f[idx_otp_lbl], f[idx_otp_in]), LoginField::Otp);
    }
    register_field(f[idx_save], LoginField::SaveEmail);
    register_field(f[idx_lock], LoginField::AutoLock);
    register_field(f[idx_keep], LoginField::KeepSession);

    let server_dirty = app.login.server_input.text().trim() != app.login.server_committed;
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
    let server_foc = app.login.active_field == LoginField::Server;
    frame.render_widget(
        Paragraph::new(Line::from(editor_spans(
            &app.login.server_input,
            server_foc,
            t,
        )))
        .block(rounded_block(focus_border(server_foc, t.accent))),
        f[2],
    );

    frame.render_widget(
        Paragraph::new("Email:").style(Style::default().fg(t.dim)),
        f[3],
    );
    let email_foc = app.login.active_field == LoginField::Email;
    frame.render_widget(
        Paragraph::new(Line::from(editor_spans(
            &app.login.email_input,
            email_foc,
            t,
        )))
        .block(rounded_block(focus_border(email_foc, t.accent))),
        f[4],
    );

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("Master Password:", Style::default().fg(t.dim)),
            Span::styled(
                "  (F2: reveal)",
                Style::default().fg(if app.login.password_visible {
                    t.accent
                } else {
                    t.dim
                }),
            ),
        ])),
        f[5],
    );
    let pass_foc = app.login.active_field == LoginField::Password;
    let pass_line = if app.login.password_visible {
        Line::from(editor_spans(&app.login.password_input, pass_foc, t))
    } else {
        Line::from(editor_spans_masked(&app.login.password_input, pass_foc, t))
    };
    frame.render_widget(
        Paragraph::new(pass_line).block(rounded_block(focus_border(pass_foc, t.accent))),
        f[6],
    );

    if app.login.awaiting_code() {
        let (label_main, label_hint) = if app.login.two_factor_required {
            let hint = match app.login.two_factor_method {
                crate::domain::TwoFactorMethod::Authenticator => {
                    "  (TOTP from your authenticator app)"
                }
                crate::domain::TwoFactorMethod::Email => "  (sent to your email)",
                crate::domain::TwoFactorMethod::YubiKey => "  (touch your YubiKey)",
            };
            ("Two-step Code:", hint)
        } else {
            ("Verification Code:", "  (sent to your email)")
        };
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(label_main, Style::default().fg(t.dim)),
                Span::styled(label_hint, Style::default().fg(t.dim)),
            ])),
            f[idx_otp_lbl],
        );
        let otp_foc = app.login.active_field == LoginField::Otp;

        let inner = Line::from(editor_spans(&app.login.otp_input, otp_foc, t));
        let block = rounded_block(focus_border(otp_foc, t.accent));
        frame.render_widget(Paragraph::new(inner).block(block), f[idx_otp_in]);

        if app.login.two_factor_required && otp_foc {
            let method_line = Line::from(vec![Span::styled(
                format!(
                    " Method: {} · ← → to cycle (Authenticator / Email / YubiKey)",
                    app.login.two_factor_method.label()
                ),
                Style::default().fg(t.dim),
            )]);

            if matches!(app.action_state, crate::tui::action::ActionState::Idle)
                && !app.login.login_error
            {
                frame.render_widget(Paragraph::new(method_line), f[idx_strip]);
            }
        }
    }

    render_checkbox(
        frame,
        "Save email",
        app.login.save_email,
        app.login.active_field == LoginField::SaveEmail,
        t.accent,
        t.inactive,
        f[idx_save],
    );
    let lock_label = format!("Auto-lock after {} min", app.auto_lock.after_secs / 60);
    render_checkbox(
        frame,
        &lock_label,
        app.auto_lock.enabled,
        app.login.active_field == LoginField::AutoLock,
        t.accent,
        t.inactive,
        f[idx_lock],
    );
    render_checkbox(
        frame,
        "Keep session",
        app.login.keep_session,
        app.login.active_field == LoginField::KeepSession,
        t.accent,
        t.inactive,
        f[idx_keep],
    );

    let strip_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(t.muted));
    if app.login.login_error {
        let msg = if app.login.two_factor_required {
            "Invalid two-factor code. Please try again."
        } else if app.login.otp_required {
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
    } else if app.login.two_factor_required {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", app.icons.two_factor()),
                    Style::default().fg(t.accent).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Open your Authenticator and enter the 6-digit code.",
                    Style::default().fg(t.accent),
                ),
            ]))
            .block(strip_block),
            f[idx_strip],
        );
    } else if app.login.otp_required {
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    format!(" {} ", app.icons.email()),
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

    render_cmd_bar_with_help(
        frame,
        bar_chunk,
        &[("Tab", "field"), ("Enter", "login"), ("F2", "reveal pwd")],
        t,
    );
}
