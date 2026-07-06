//! Login / unlock screen renderer.

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
    focus_border, input_with_cursor, render_checkbox, render_cmd_bar_with_help, rounded_block,
};

thread_local! {
    /// Frame-local hit map for the login form — one `(rect, field)` per
    /// clickable field, recorded from the exact layout rects as the form
    /// draws (so a click lands on the field the user sees, with no
    /// re-derived row arithmetic that can drift from the renderer).
    static LOGIN_HITS: std::cell::RefCell<Vec<(Rect, LoginField)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn register_field(rect: Rect, field: LoginField) {
    if rect.width > 0 && rect.height > 0 {
        LOGIN_HITS.with(|h| h.borrow_mut().push((rect, field)));
    }
}

/// The login field under `(column, row)`, if any — consumed by the mouse
/// layer to focus / toggle the field the pointer is over.
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

/// Unions two vertically-adjacent rects (a field's label row + its input
/// box) so a click on either focuses the field.
fn union(a: Rect, b: Rect) -> Rect {
    let x = a.x.min(b.x);
    let y = a.y.min(b.y);
    let right = (a.x + a.width).max(b.x + b.width);
    let bottom = (a.y + a.height).max(b.y + b.height);
    Rect::new(x, y, right - x, bottom - y)
}

/// Renders the login screen.
pub fn draw(frame: &mut Frame, app: &mut App) {
    // While a request is in flight (logging in / loading the vault) the
    // form has nothing actionable — show the same centered logo + spinner
    // as the boot/session-check splash instead of the form with a loading
    // line tacked underneath.
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

    // Form rows: padding(1)+server-lbl(1)+server-in(3)
    //            +email-lbl(1)+email-in(3)+pass-lbl(1)+pass-in(3)
    //            + [otp-lbl(1)+otp-in(3)]
    //            +save(1)+lock(1)+keep_session(1)+strip(2)+border(2).
    let form_height: u16 = if app.login.awaiting_code() { 24 } else { 20 };

    // Vertical layout — stars above the form (2/3) and below (1/3),
    // command bar at the bottom.
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

    // Build the inner vertical splits dynamically — OTP rows only exist
    // when needed (device verification *or* permanent 2FA).
    let (idx_otp_lbl, idx_otp_in, idx_save, idx_lock, idx_keep, idx_strip, f);
    if app.login.awaiting_code() {
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
    // Register each field's clickable area (label row + input box) from the
    // exact layout rects, so the mouse focuses whatever the user points at.
    register_field(union(f[1], f[2]), LoginField::Server);
    register_field(union(f[3], f[4]), LoginField::Email);
    register_field(union(f[5], f[6]), LoginField::Password);
    if app.login.awaiting_code() {
        register_field(union(f[idx_otp_lbl], f[idx_otp_in]), LoginField::Otp);
    }
    register_field(f[idx_save], LoginField::SaveEmail);
    register_field(f[idx_lock], LoginField::AutoLock);
    register_field(f[idx_keep], LoginField::KeepSession);

    // ── Server ────────────────────────────────────────────────────────────
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
        Paragraph::new(input_with_cursor(
            app.login.server_input.text(),
            app.login.server_input.cursor(),
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
    let email_foc = app.login.active_field == LoginField::Email;
    frame.render_widget(
        Paragraph::new(input_with_cursor(
            app.login.email_input.text(),
            app.login.email_input.cursor(),
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
        input_with_cursor(
            app.login.password_input.text(),
            app.login.password_input.cursor(),
            pass_foc,
            t,
        )
    } else {
        let masked_before = "●".repeat(app.login.password_input.cursor());
        let masked_after = "●".repeat(
            app.login
                .password_input
                .len_chars()
                .saturating_sub(app.login.password_input.cursor()),
        );
        if pass_foc {
            Line::from(vec![
                Span::raw(masked_before),
                Span::styled("█", Style::default().fg(t.accent)),
                Span::raw(masked_after),
            ])
        } else {
            Line::from(Span::raw("●".repeat(app.login.password_input.len_chars())))
        }
    };
    frame.render_widget(
        Paragraph::new(pass_line).block(rounded_block(focus_border(pass_foc, t.accent))),
        f[6],
    );

    // ── OTP / 2FA ────────────────────────────────────────────────────────
    if app.login.awaiting_code() {
        let (label_main, label_hint) = if app.login.two_factor_required {
            // Method-specific hint so the user knows what code is
            // being asked for. The method chip below the label
            // shows the current selection + cycle hint.
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
        // For 2FA we render a compact method chip on the input row's
        // right side so the user can tell at a glance which factor
        // is active. Cycling happens via ← → when focus is on the
        // Otp field.
        let inner = input_with_cursor(
            app.login.otp_input.text(),
            app.login.otp_input.cursor(),
            otp_foc,
            t,
        );
        let block = rounded_block(focus_border(otp_foc, t.accent));
        frame.render_widget(Paragraph::new(inner).block(block), f[idx_otp_in]);

        if app.login.two_factor_required && otp_foc {
            // One-line tip below the code input — small, dim, only
            // when focused so it doesn't add noise on the rest of
            // the form.
            // We re-use the OTP-input box's row by overlaying — but
            // simpler: show it as a status hint via the existing
            // feedback strip below. Actually, simplest: append it
            // as an inline label in the code label row. Done above
            // via `label_hint`. Method chip is the next addition:
            // render it as a one-row strip just under the input.
            let method_line = Line::from(vec![Span::styled(
                format!(
                    " Method: {} · ← → to cycle (Authenticator / Email / YubiKey)",
                    app.login.two_factor_method.label()
                ),
                Style::default().fg(t.dim),
            )]);
            // Repurpose the OTP-input area's last row by re-rendering
            // a thin overlay — but we don't have a dedicated chunk
            // for it in the layout. Easiest: render it on top of the
            // input border's bottom row. Since `input_with_cursor`
            // fills the box, we instead cram the method chip into
            // the *label* row when focused, by adding a second line
            // below the existing label hint via the strip helper.
            //
            // The cleanest implementation is to just write the chip
            // into the strip area below the input. We rely on the
            // existing `idx_strip` cell — the strip already gets
            // overwritten by `action_line` further down. So we
            // intercept here only when nothing else is showing.
            if matches!(app.action_state, crate::tui::action::ActionState::Idle)
                && !app.login.login_error
            {
                frame.render_widget(Paragraph::new(method_line), f[idx_strip]);
            }
        }
    }

    // ── Checkboxes ────────────────────────────────────────────────────────
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

    // ── Feedback strip ────────────────────────────────────────────────────
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
                    " 🔐 ",
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
        "Tab field · Enter login · F2 reveal pwd",
        "Tab  Enter  F2",
        t.dim,
        t,
    );
}
