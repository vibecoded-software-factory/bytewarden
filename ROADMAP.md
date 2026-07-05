# ROADMAP — restructuring

Working plan for the complete restructuring of bytewarden toward the target
architecture in [`CLAUDE.md`](CLAUDE.md) and [`UX.md`](UX.md). Each batch is
one branch → PR → squash-merge to `dev`.

> This file is a temporary work tracker. **It is deleted once Batch 15
> lands** — the spec (`CLAUDE.md` / `UX.md`) is the durable source of truth.

## Foundations

- [x] **Batch 1 — Normative spec.** Rewrite `CLAUDE.md` and `UX.md` as the
  target of the restructuring. *(PR #5)*
- [x] **Batch 2 — `BwError` (typed errors).** Added `ports/error.rs` with
  `BwError` (`Spawn` · `Timeout{label,secs}` · `Exit{stderr,status}` ·
  `InvalidJson` · `Shape` · `Internal`). Every port method moved from
  `Result<_, String>` to `Result<_, BwError>`, classified at the boundary
  (`bw_cli/mod.rs`, `process.rs`); `Display` + `std::error::Error`; the
  command log renders the classified error. Login challenges stay in the
  domain `LoginOutcome` (an `Auth` variant is deferred — see CLAUDE.md).
- [x] **Batch 3 — Execution discipline (`App::submit`).** `App::submit(slot,
  label, req)` = `begin` + `Running` toast + send; a failed send routes
  through `on_worker_dead`. Added `begin` (refuses double-send / dead worker),
  `on_worker_dead`, `watchdog_release_stuck_request` (per tick, budget from
  the list-items timeout). Run loop detects `TryRecvError::Disconnected` →
  `on_worker_dead`. Every `request_*` migrated to `submit` (non-silent) or
  bare `begin` (silent reloads); the fire-and-forget lock stays a bare send.
  New unit tests cover the single-in-flight / worker-dead / dispatch
  invariants.
- [x] **Batch 4 — `LineEditor` + shared input.** Added `domain::LineEditor`
  (UTF-8-safe char-index cursor, word ops `Ctrl+W`/`Ctrl+U`/`Ctrl+←→`/
  `Ctrl+A`/`Ctrl+E`, `ZeroizeOnDrop`) and `input::common::route_line_editor`.
  Migrated all **8 popup** text inputs (export · import · send · reprompt ·
  folder-name · rename-field · attachment up/download) onto it, deleting the
  8 hand-rolled char-index editors; `widgets::editor_line` renders from the
  editor. LineEditor + router are unit-tested.
  - *Deferred:* the **login** fields (`server`/`email`/`password`/`otp`) keep
    their `app.rs` editors for now — they carry side-effects (save-email
    persistence, 2FA method cycling, password masking) that warrant a
    separate pass; folded into a later input batch.
  - *Deferred:* the remaining `input::common` helpers (`search_key`,
    `list_nav`, `confirm_key`/`run_confirm`) land with Batches 5/7 where the
    list/confirm/search surfaces are reworked.

## UI system

- [x] **Batch 5 — Chrome styling centralization (`view/widgets.rs`).** Added
  `Theme::emphasis()` / `danger_title()` and `widgets::focus_style` /
  `key_style`; refactored `titled_block` to take an explicit `focused: bool`
  — **killing the `col == accent` color-equality hack**. Consolidated the two
  centering helpers (`center_rect` rows + `center_rect_pct` percent; help.rs's
  private `centered` removed). Fixed the spec's border rule to match
  bytewarden's real convention (square section panels, rounded popups/cards).
  Swept **all cross-project references** out of the code comments (hard-rule
  cleanup). Theme helpers unit-tested.
  - *Redistributed* (the remaining widget vocabulary lands where each surface
    is actually reworked): `draw_confirm_popup` + `run_confirm` → Batch 7 (the
    confirm mechanics ride with the keybinding gradient); `empty_state_lines`
    + `draw_scrollbar` + `legend_line` → Batch 12 (responsiveness);
    `draw_picker_modal` → Batch 14 (the command palette needs it);
    `list_table` when a second multi-column table appears (today the vault
    list is the only one).
- [x] **Batch 6 — Condition badge + sticky errors.** Added the persistent
  **`⚠ WORKER DEAD`** condition badge to the `─[0]-Status` panel (survives
  keypresses; completes the Batch 3 worker-dead story). Made **error toasts
  sticky** — they persist until the next keypress clears them
  (`input::handle_events`); success toasts keep the ~1.5 s fuse (`tick_state`).
  - *Not applicable to bytewarden* (dropped, not deferred): the **nvim-style
    mode badge** (bytewarden navigates by focused panel, not editor modes) and
    the **3-tier `disabled_block`** (every vault panel is always reachable, so
    there's no "unavailable" tier). UX.md reconciled to the real layout
    (sidebar `[0]-Status` + hint bar, no bottom mode strip).
- [x] **Batch 7 — Keybinding gradient.** Adopted the gradient (user-approved
  map): **bare letters act** on the non-typing surfaces (List `n/e/c/u/f/x/d/r`,
  Folders `n/r/d`, Detail-read `c/e/m/d/x/a/s/r`); `Alt+letter` = app-wide
  commands (sync/export/import/send/memberships/fingerprint/generator/lock/
  logout — bytewarden focuses panels with `0`–`4`, so `Alt` is free); the
  Search box types, so its row actions ride on `Alt`; delete is bare `d`
  (confirm-gated), `Shift+D` = permanent. The old `Alt+letter` row shortcuts
  stay as transition aliases; **bare is canonical**. The `Alt+S/L/R/D`
  collisions dissolve (row/panel actions moved to bare, context = focused
  panel). Synced all four surfaces: footer hints · `view/help.rs` · `README`
  tables · `UX.md`.

## State & coherence

- [ ] **Batch 8 — Invalidation contracts.** Formalize the footgun table: one
  rebuild path per cache, cursor re-anchored **by id** after a reload.
  Decompose the `App` god-object (popup state as sub-structs / per-screen
  state, not flat `Option<XState>`).
- [ ] **Batch 9 — Unified field model (detail/edit).** One source of truth for
  field order (today `detail_fields.rs` and `edit_field.rs` diverge). Kill the
  hand-walked positional field enumeration in `copy_selected_field`.
- [ ] **Batch 10 — Background lane.** Silent reloads (post-delete/import/move,
  idle resync) off the user's slot, routed by response variant.

## Theme, responsiveness, robustness

- [ ] **Batch 11 — Theme.** Legibility hierarchy (5 tiers): `inactive`/`dim`
  lifted out of the dark band; list rows render `foreground`, not `dim`.
  `ColorCaps::detect` + `adapt` (NO_COLOR mono, xterm-256 quantization,
  truecolor passthrough). More presets; fix the stale "Mocha" vs Nord-default
  comment.
- [ ] **Batch 12 — Responsiveness.** Monotonic `cmdlog_height`, `fit_segments`
  footer, scrollbars on every overflowing region, wrapped field values,
  modals that window by real height.
- [ ] **Batch 13 — Adapter robustness.** OSC 52 fallback + compare-and-clear
  in the clipboard. Atomic settings writes (temp + `rename`) + 0600/0700
  perms. Consistent timeouts (today `bw_generator` calls `Command` without
  one). Tolerant parsing with `skipped` diagnostics (never drop the whole
  list on one bad row). Move `send_text` content out of argv.
- [ ] **Batch 14 — Command palette (`Ctrl+P`).** Context-aware palette over
  `palette_commands`, doubling as an executable cheat-sheet; the 5th
  keybinding-sync surface.
- [ ] **Batch 15 — Final cleanup.** Stale annotations (`Item::folder_id`
  `dead_code`, `VaultInfo::last_sync/server_url`, `check_name_unique` dead
  computation, `bold()`/`target_kind()`, hardcoded `VERSION`). Test coverage
  for all new pure logic on `App`/`domain`. **Delete this `ROADMAP.md`.**

## Open decisions (settle before touching them)

- **Keybinding gradient** (Batch 7) — changes today's muscle memory
  (`Alt+`-for-everything). Confirm before implementing.
- **Decompose `App`** (Batch 8) — how far: sub-structs vs a full per-screen
  state machine.
- **Command palette** (Batch 14) — ship it as a new feature, yes/no.
- **`#![forbid(unsafe_code)]`** — adopt crate-wide in the rewrite, yes/no.
