# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] — 2026-05-03

First public release.

### Item management
- All five Bitwarden item types: Login, Secure Note, Card, Identity, SSH Key.
- Inline edit mode with multi-URI logins, custom fields (text / hidden / boolean),
  type cycling and field rename popup.
- Custom fields of type `linked` from the official Bitwarden GUI are preserved
  read-only across edits — the patcher keeps `linkedId` intact instead of
  silently dropping it.
- Trash / restore / permanent delete with confirmation popups.

### Folders, send, attachments, memberships
- Full folder CRUD with inline create / rename / delete popups.
- Bitwarden Send (text) creation with auto-copy of the URL on success.
- Attachment upload, **download** (path picker pre-filled to `~/Downloads/`) and
  **delete** (confirmation popup).
- Read-only memberships view (organisations + collections).

### Auth & session
- Three login methods — master password, headless API-key, SSO browser flow.
- "New device" OTP detection with **stdin-fed** verification code (no `--code`
  in `argv`, never visible in `ps`).
- Session persistence behind a `Keep session` checkbox: per-PPID file at
  `${XDG_RUNTIME_DIR}/bytewarden/session-<ppid>` (mode `0600`), wiped when the
  parent shell dies or after 24 h regardless of liveness.

### Security hardening
- Wall-clock timeouts on every `bw` call that touches the network — no more
  30-second TUI freezes when the server is slow.
- Master password fed via `BW_PASS_INPUT` env var, OTP via stdin — neither
  value reaches `argv`.
- Session key, password buffer and OTP buffer wrapped in `zeroize::Zeroizing`
  — overwritten with zeroes on drop.
- `~/.config/bytewarden/config.toml` created atomically with `0600` mode via
  `OpenOptions::mode`; the directory is `0700`.
- `cmd_log` is wiped on logout.
- Pre-flight input validation on email, server URL, export/import paths,
  folder names and custom-field labels — clear toasts before paying for a
  network round-trip.
- Zero `unsafe` blocks in production code.

### TUI
- Hexagonal architecture: domain / ports / adapters / TUI driving adapter.
- Themable via `[theme]` block in `config.toml` (18 colour keys, all optional).
- Per-screen scoped help popup (F1) with vertical and horizontal scrolling.
- Mouse capture: click panels, click items, scroll wheel everywhere,
  Shift+wheel for horizontal pan in the help popup.
- Auto-lock after configurable inactivity.
- Fuzzy search across name / username / URI with weighted scoring.
- Optional persistent debug log via `BYTEWARDEN_DEBUG=1` →
  `~/.bytewarden.log`.
- Graceful "terminal too small" fallback when the area is below 60×18.

### Quality
- 190 unit tests + 4 doctests passing.
- ~95 % line coverage on the testable layer (domain, port-pure adapters, TUI
  helpers, validation, session-file).
- `cargo clippy -D warnings` and `cargo fmt --check` clean.
- GitHub Actions CI on every push and PR.

[1.0.0]: https://github.com/51lv3str1/bytewarden/releases/tag/v1.0.0
