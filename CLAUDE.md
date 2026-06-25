# CLAUDE.md

Guidance for Claude Code when working in this repository.

## UX/UI — read `UX.md` first (hard rule)

**Before any UI edit or new feature, read [`UX.md`](UX.md)** — it is the
canonical design system (screen layout, the shared `view::widgets`
components, chrome, confirm overlays, keybinding conventions, theme).
Build new UI by reusing the documented components, never a one-off. **Keep
`UX.md` updated before every push** when a change touches UX/UI (new screen,
new pattern, changed convention): update the spec in the same change and
apply it everywhere.

## Working agreements (READ FIRST)

1. **Fix every occurrence, not just the one reported.** When the user reports
   a problem (a bug, a wording issue, a layout glitch, a missing keybinding,
   …), treat the reported spot as one *instance* of a class. Before finishing,
   grep the whole app for the same pattern and fix it everywhere it appears.
   - Practical step: after a targeted fix, `grep` for the literal/string/
     pattern across `src/` and confirm there are no siblings left.

2. **Every UX change must stay coherent with the rest of the UI.** This app
   has established, repeated patterns; a change to one screen should match all
   the others, and ideally reuse the same component.
   - Bordered panel → `tui::view::widgets::titled_block` (top-left title +
     dim bottom-right counter) / `rounded_block`. Focus tint via
     `widgets::focus_color` / `focus_border`.
   - Bottom hint bar → `widgets::render_cmd_bar_with_help`: short per-focus
     navigation hints on the left, **`F1: help` anchored right** (never
     truncated). Popups use the plain `render_cmd_bar` (self-contained
     instructions, no F1). The full key list lives in the help popup, not the
     bar.
   - Help popup → `tui::view::help::draw_popup` is **context-aware**: it shows
     only the shortcuts for the originating screen (`App::help_from`) and, on
     the vault, the focused panel (`App::focus`). Any new screen/keybinding
     must be added to the matching section in `help.rs` (+ its `screen_label`).
   - Text inputs → `widgets::input_with_cursor` / `cursor_line` (block
     cursor); checkboxes → `widgets::render_checkbox`; field cards (detail /
     edit / create) → `widgets::field_areas` + `render_field_card`; centered
     popups → `widgets::center_rect`.
   - List filtering follows the `App::filtered_cache: Vec<usize>` +
     `search_query` + `rebuild_filtered_cache()` convention, ranked with
     `domain::search::fuzzy_score_lowered` (pure helper
     `compute_filtered_indices`). Selection (`selected_index`) indexes the
     **filtered** cache, never the raw `items` vec. The `url:` query prefix
     narrows to login URIs.
   - Global keys are consistent across screens (`/` focus search, `Esc`/`h`
     back, `F1`/`?` help, `0`–`4` focus panel, `Tab` cycle). **Only `Ctrl+C`
     quits.** Actions use the **`Alt+<letter>`** convention (e.g. `Alt+C`
     copy, `Alt+E` edit, `Alt+N` new) — see `UX.md`.

3. **Verify before declaring done.** Run `cargo build`, `cargo clippy
   --all-targets -- -D warnings` (must be warning-free) and `cargo test`
   after every change. Add/adjust unit tests for new pure logic on
   `App`/`domain`.

## What this is

`bytewarden` — a terminal UI (Ratatui) over the **Bitwarden CLI**. Flow:
login / unlock → vault list (sidebar + search + list) → item detail →
edit/create. Full CRUD over items (5 types), folders, attachments, Sends,
import/export, plus generator, memberships, HIBP check. It shells out to the
`bw` binary and parses its JSON; there is no Bitwarden SDK dependency.

## Before every commit (no exceptions)

```sh
cargo fmt --all
cargo clippy --all-targets -- -D warnings   # must be warning-free
cargo test
```

Run this even on a one-line or comment-only change. `cargo fmt` is the
formatter of record (default rustfmt; there is no `rustfmt.toml`, so don't
hand-format against the default style). Clippy is a hard gate — warnings are
failures here, not suggestions (CI sets `RUSTFLAGS: -D warnings`). Keep the
`README.md` keybinding tables and the `view/help.rs` popup in sync when
shortcuts change.

## Stack

- **Rust, edition 2024**, toolchain pinned in `rust-toolchain.toml`
  (`1.95.0`, with `clippy` + `rustfmt` + `llvm-tools-preview`). Don't bump
  the channel as a side effect.
- **No `unsafe` in the composition root.** `main.rs` carries no `unsafe`
  blocks — the keep-session seed is passed to `BwCliAdapter::new_with(seed)`
  instead of `std::env::set_var` (which is `unsafe` in edition 2024). There
  is **no** crate-wide `#![forbid(unsafe_code)]`; don't add new `unsafe`
  without an explicit ask.
- **Ratatui 0.30** + **Crossterm 0.29** for the TUI and terminal events.
- **Serde / serde_json** to parse `bw` JSON output.
- **color-eyre** for error reports, **figlet-rs** for the login wordmark
  (bundled `slant.flf`, no system `figlet` needed), **zeroize** to wipe
  credential-bearing buffers.
- `tempfile` is a dev-dependency only (test fixtures).
- Release profile is size-optimized (`opt-level = "s"`, `lto`, `strip`).

## Execution model — worker thread + mpsc (do NOT touch unprompted)

The vault + generator ports live on a **single worker thread** that owns them
and serves requests serially over `mpsc`; the render thread never blocks on a
`bw` call (`tui/worker.rs`). The flow:

1. A `request_*` builder (in `tui/flows/`) validates input, stashes a
   `worker::InFlight` ticket on `App::in_flight`, sets a `Running` toast, and
   sends a `WorkerRequest` on `app.worker_tx`.
2. The worker runs the blocking `bw` call off-thread and sends a
   `WorkerResponse` back. Each call is wrapped in `run_caught` so a panic in
   one can't kill the worker.
3. The run loop (`tui/mod.rs::run_loop`) drains `app.worker_rx` every frame and
   routes each response through `flows::apply_response`, which consumes the
   `in_flight` ticket and dispatches to the owning `handle_*`. The spinner
   animates throughout; `Ctrl+C` stays instant.

Multi-step flows (login → load → session-data; save = fetch → patch → edit;
delete → reload) **chain** by having a `handle_*` queue the next `request_*`.
Only one user request is in flight at a time (`App::in_flight: Option<_>`);
`input::busy_blocks` gates all keys but `Esc` while busy so a second request
can't be queued. Each call still has a per-operation timeout in
`adapters/bw_cli/process.rs`.

Clipboard + settings stay **synchronous** on the render thread (they're fast).
Plain clipboard copies (username/password) read an in-memory item and don't
touch the worker; only TOTP / HIBP do.

**Do NOT pull in `tokio`/`async-std`.** Extend the `std::thread` + `mpsc`
worker pattern: add a `WorkerRequest`/`WorkerResponse`/`InFlight` variant and a
`request_*`/`handle_*` pair.

## No Bitwarden SDK

All Bitwarden access is the `bw` CLI binary spawned as a subprocess
(`adapters/bw_cli/`). Adding functionality means a new CLI invocation, not an
SDK crate: build argv in `codec.rs`, run with a timeout in `process.rs`,
parse in `json.rs`. Master passwords / OTP / 2FA codes are fed via
stdin / the `BW_PASS_INPUT` env var, **never** argv (so they don't appear in
`ps`). Every invocation is appended to the in-app command log with the
session key redacted (`***`).

## Architecture (hexagonal / ports & adapters)

```
main ──► tui ──► flows ──► ports ◄── adapters
                    ▲
                    └── domain (pure types, no I/O)
```

- `src/domain/` — pure types and rules, no I/O (e.g. `Item`, `LoginData`,
  `Folder`, `Collection`, `ItemFilter`, `fuzzy_score_lowered`, validators,
  fingerprint phrase).
- `src/ports/` — trait abstractions: `VaultPort`, `ClipboardPort`,
  `SettingsPort`, `PasswordGeneratorPort`.
- `src/adapters/` — the only layer allowed to do I/O: `bw_cli/` (subprocess +
  `codec` + `process` + `json`), `bw_generator.rs`, `clipboard_system.rs`
  (`wl-copy`/`xclip`/`xsel`/`pbcopy`), `settings_toml.rs` (hand-rolled TOML
  that preserves unknown keys).
- `src/tui/` — the driving adapter:
  - `app.rs` — global mutable `App` state container (incl. `worker_tx`/
    `worker_rx`/`in_flight`).
  - `worker.rs` — the worker thread + `WorkerRequest`/`WorkerResponse`/
    `InFlight` enums + `WorkerHandle`.
  - `action.rs` — `ActionState` (Idle/Running/Done/Error) + `CmdEntry`.
  - `screens.rs` — `Screen`, `Focus`, `LoginField` enums.
  - `flows/` — per-feature `request_*`/`handle_*` pairs (`auth`, `vault`,
    `items`, `copy`, `generator`, `folders`, `memberships`, `reprompt`, …).
    `flows::apply_response` routes each `WorkerResponse` to its `handle_*`.
  - `input/` — per-screen keyboard handlers (wired in `input/mod.rs::
    handle_events`) + `mouse.rs` + shared `nav.rs`.
  - `view/` — per-screen Ratatui renderers (router in `view/mod.rs::draw`) +
    shared chrome in `view/widgets.rs`; `theme.rs`, `logo.rs`, `starfield.rs`.
  - Helpers: `detail_fields.rs` (detail row model), `edit_field.rs` (edit/
    create field model), `folders.rs` (`FolderFilter` + sidebar), `import.rs`/
    `export.rs`/`send.rs`/`assign_collections.rs`/`reprompt.rs` (popup state),
    `session_file.rs` (keep-session per-PPID file), `debug_log.rs`.

When adding a new screen: add the `Screen` variant, an `input/<screen>.rs`
handler (wired in `input/mod.rs`), a `view/<screen>.rs` renderer (wired in
the `view/mod.rs::draw` router — popups draw their origin screen underneath
first), reuse `widgets::*` for chrome, and add its section to `view/help.rs`.
**Reuse the shared helpers above rather than re-implementing per screen** —
that's what keeps the app coherent.

## Security (Bitwarden-specific)

- **Secrets live in `zeroize`d buffers.** The session key, master-password
  buffer, OTP/2FA buffer, and every vault payload (`Item` & friends) are
  `Zeroizing` / `ZeroizeOnDrop`; the in-memory cache is wiped on lock /
  logout / shutdown. JSON intermediates and edit-form / generator buffers are
  `Zeroizing<String>` too.
- **Clipboard auto-clear** (`clipboard_clear_secs`, default 30 s) wipes any
  copied secret — only if the clipboard still holds bytewarden's write.
- **Reprompt** re-verifies the master password before exposing a secret on a
  reprompt-flagged item (copy password/TOTP/hidden field, F2 reveal); routed
  through `flows::reprompt`. No caching — every protected action re-prompts.
- **Auto-lock** after inactivity (`lock_after_minutes`); **keep-session**
  writes the session key to a per-PPID file (`tui/session_file.rs`, mode
  0600). Config file is mode 0600. `BYTEWARDEN_DEBUG=1` appends the redacted
  command log to `~/.bytewarden.log`.
- Don't add a surface that writes secrets to disk (beyond the user-chosen
  export path) without an explicit ask.

## Branching

- The default / deploy branch is **`dev`** — never commit directly to it.
- Work on `feat/<short-kebab>`, `fix/<short-kebab>` or `hotfix/<short-kebab>`
  branches; open PRs against `dev`.
- Branch first if you find yourself on `dev` before making changes.
- CI (`.github/workflows/ci.yml`) runs fmt + clippy + build + test on
  `main` and `dev`.

## Commits

- **Conventional Commits** prefix: `feat:` · `fix:` · `refactor:` · `docs:` ·
  `test:` · `chore:` · `ci:` · `style:` · `perf:` · `build:`.
- Subject ≤ 72 chars. Body explains the **why**, not the what. One logical
  change per commit.
- Only commit or push when the user asks.
- **Do NOT append `Co-Authored-By: Claude …` or any AI / generated-by trailer
  to commits, and no "🤖 Generated with Claude Code" footer in PR bodies.**
  The Claude Code default adds these; this rule overrides that default. Don't
  add them unless explicitly asked.

## Things to NOT touch unprompted

- The worker-thread + `mpsc` execution model — keep `bw` calls off the render
  thread; extend the `WorkerRequest`/`WorkerResponse`/`InFlight` + `request_*`/
  `handle_*` pattern, don't reach for `tokio`/`async-std`.
- The ports/adapters boundary — I/O (subprocess, filesystem, clipboard)
  belongs only in `adapters/`; keep `domain/` pure.
- Existing keybindings and the shared chrome widgets — changing one screen's
  UX means changing all of them for coherence (see Working agreements), not a
  one-off divergence.
- The secret-handling discipline (zeroize, reprompt, redacted logging, no
  secrets in argv / on disk).

## Commands

```sh
cargo run                         # run the TUI (needs `bw` on PATH)
cargo build                       # debug build
cargo build --release             # optimized binary at target/release/bytewarden
cargo clippy --all-targets -- -D warnings   # lint (keep warning-free)
cargo test                        # unit tests
```
