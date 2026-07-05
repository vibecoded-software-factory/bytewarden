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
- [ ] **Batch 4 — `LineEditor` + shared input.** `domain::LineEditor`
  (UTF-8-safe byte cursor, word ops `Ctrl+W`/`Ctrl+U`/`Ctrl+←→`/`Ctrl+A`/
  `Ctrl+E`, `ZeroizeOnDrop`). `input::common`: `route_line_editor`,
  `search_key`, `list_nav`/`list_nav_arrows`, `confirm_key` + `run_confirm`,
  `busy_blocks`, `cycle_focus`. Delete the ~8 copies of the char-index editor
  in the popups + the login variant.

## UI system

- [ ] **Batch 5 — Widget vocabulary (`view/widgets.rs`).** `list_table` +
  `list_title` + `table_row_at`; `draw_picker_modal(PickerModal{..})` with a
  per-frame hit map; `draw_confirm_popup` + `run_confirm` (one confirm look,
  default = cancel); `draw_input_popup`, `legend_line`, `button`,
  `empty_state_lines`, `draw_scrollbar`; `focus_style`/`key_style`/`MODAL_*`,
  `Theme::emphasis()`/`danger_title()`. Kill the `titled_block`
  focus-via-color-equality hack; route status/search/cmdlog/settings through
  `titled_block`. Consolidate the 3 centering helpers into one.
- [ ] **Batch 6 — 3-tier borders + status strip.** `disabled_block` (the
  `muted` "unavailable" tier) alongside focused/available. `draw_status_strip`:
  mode badge · condition badges (`⚠ WORKER DEAD`) · feedback · hint · `F1`
  anchor. Sticky errors (cleared by the next keypress; successes keep the
  ~1.5 s fuse).
- [ ] **Batch 7 — Keybinding gradient.** bare letter = action on the focused
  list · `Shift` = destructive/loud · `Ctrl` = global · `Alt` = panel jump +
  compose verbs · `/` = search. Resolve the current `Alt+S/L/R/D` collisions.
  Central keybinding registry; sync the 4 surfaces (footer · `help.rs` ·
  `README` · `UX.md`).

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
