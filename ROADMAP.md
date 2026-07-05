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

- [x] **Batch 8 — Invalidation contract (cursor re-anchor by id).** Added
  `App::selected_item_id` + `App::reanchor_selection(id)` and the
  `flows::vault::set_items_keep_cursor` wrapper; wired every background /
  post-mutation refresh (silent reload, sync, import, move, folder-delete,
  manual load) through it so the cursor follows the same item **by id** across
  a reorder/reload instead of jumping to whatever index it held. The
  intentional-reset paths (create → new item, restore → top) keep plain
  `set_items`. Unit-tested (follows-by-id + clamps-when-gone).
  - *Deferred:* decomposing the `App` god-object into per-screen sub-structs
    is a large, high-churn, low-visible-ROI refactor — parked until a
    concrete need (or a quieter moment) rather than risking wide regressions
    blind. The one rebuild-path-per-cache discipline is already documented in
    CLAUDE.md and honored.
- [x] **Batch 9 — Kill the hand-walked copy enumeration.** Replaced
  `copy_selected_field`'s brittle `idx += 1` walk (which duplicated the detail
  renderer's field order and silently forgot attachment rows) with a pure
  `detail_copy_targets(item) -> Vec<CopyTarget>` that mirrors
  `build_detail_fields` row-for-row; the copy handler just indexes it and
  dispatches by target kind. A unit test **pins** the two to the same length
  (login/card/ssh), so they can't drift again — and attachment rows are now
  handled explicitly (they copied nothing before).
  - *Deferred:* fully merging `detail_fields.rs` and `edit_field.rs` into one
    builder is a large model change (the two are different projections — masked
    read display vs editable form with per-field kind/cursor), high-risk to do
    blind. The copy-order divergence (the concrete bug) is fixed and pinned;
    the broader merge waits for a runtime-verifiable moment.
- [~] **Batch 10 — Background lane. (Deferred — low ROI for bytewarden.)**
  The second worker lane exists in the north star to serve the *idle inbox
  resync* and the *push stream* — **bytewarden has neither** (the vault only
  changes via the user's own actions or an explicit sync). The post-mutation
  silent reloads gate input only briefly, right after a user action. A second
  worker thread + variant-based response routing carries real threading risk
  for a marginal benefit, so it's parked. Revisit if bytewarden ever adds a
  periodic auto-sync.

## Theme, responsiveness, robustness

- [x] **Batch 11 — Theme: terminal color-capability adaptation.** Added
  `ColorCaps::{Mono, Indexed256, True}` + `detect()` (from `NO_COLOR` /
  `COLORTERM`) and `theme::adapt`, applied at boot (`load`) and in the live
  picker: `NO_COLOR` collapses every hue to a grayscale tier (`to_gray`), a
  non-truecolor terminal gets every RGB **deterministically quantized** to the
  nearest xterm-256 index (`quantize_256`, cube vs ramp by squared error), and
  truecolor passes through. `foreground: Reset` survives every mode. A
  `map_colors` lists every field explicitly so none can skip adaptation. Fixed
  the stale "Mocha"/"three TUIs" theme comments and swept the **last
  cross-project references** out of `src/`. Pure functions unit-tested.
  - *Deferred:* the legibility-hierarchy re-derivation (lifting `inactive`/
    `dim` toward text; list rows `foreground` not `dim`) is a **visual** change
    across every screen — held until it can be verified in a real terminal,
    not shifted blind. More presets are additive and can land any time.
- [~] **Batch 12 — Responsiveness. (Deferred — needs runtime verification.)**
  Every piece here is a **visual layout change** (responsive command-log
  height, scrollbars on overflowing regions, wrapped field values, modals
  windowed by real height) whose correctness can only be judged by rendering
  it in a real terminal at various sizes. Shipping unverified layout risks
  visible breakage, so this waits for a terminal-in-the-loop session rather
  than being changed blind. (The footer already truncates via
  `render_cmd_bar_with_help`.)
- [x] **Batch 13 — Adapter robustness (internal, testable slice).** Made
  settings writes **atomic** (`write_file_secure` now writes a sibling
  `<path>.tmp` with 0600 from the first byte, `sync_all`s it, then `rename(2)`s
  over the target — a crash mid-write leaves the old config intact). Routed
  `bw_generator` through the shared `process::bw_run_timeout` so a hung
  `bw generate` can't freeze the worker (it had no timeout). Both unit-tested
  (atomic write: content + 0600 + no temp residue).
  - *Deferred (need runtime / `bw` verification):* the **OSC 52** headless
    clipboard fallback (writes escape sequences to stdout while Ratatui owns
    the screen — must be verified it doesn't corrupt the display); moving
    `send_text` content **out of argv** (needs the `bw send` stdin/file
    interface confirmed live); **tolerant per-row list parsing** with `skipped`
    diagnostics (a moderate parse-path change worth doing with real vault
    fixtures).
- [ ] **Batch 14 — Command palette (`Ctrl+P`).** Context-aware palette over
  `palette_commands`, doubling as an executable cheat-sheet; the 5th
  keybinding-sync surface.
- [x] **Batch 15 — Final cleanup.** Removed the stale `#[allow(dead_code)]` +
  wrong comment on `Item::folder_id` (it *is* used — folder filtering) and on
  `VaultInfo::server_url` (seeded into the Login form); accurate notes on the
  genuinely-unread `last_sync`. Deleted the dead `let lower` computation in
  `check_name_unique`, the unused `widgets::bold()` and `view::generator::
  target_kind()` helpers. `logo::VERSION` now derives from `CARGO_PKG_VERSION`
  (`concat!("v", …)`) so it can't drift from the build. Test coverage for the
  new pure logic landed with each batch.
  - **`ROADMAP.md` is intentionally *not* deleted yet** — the batches below
    are deferred, not done, and this file is their record. Delete it once the
    deferred backlog is resolved (or explicitly dropped).

---

## Deferred backlog (resolve before deleting this file)

Each was a deliberate call — deferred for **runtime verification** (can't judge
a visual/behavioral change without driving the TUI with `bw` in a real
terminal), for a **pending decision**, or as **low-ROI/high-churn** for
bytewarden specifically. None are blockers; the shipped batches are green
(fmt + clippy `-D warnings` + tests).

- **Whole batches:** Batch 10 (background lane — low ROI: no idle/push refresh),
  Batch 12 (responsiveness — all visual layout), Batch 14 (command palette —
  new feature + open decision).
- **Deferred halves:** decompose the `App` god-object (8); merge the
  detail/edit field builders (9); legibility-hierarchy re-derivation (11); OSC 52 clipboard + tolerant per-row
  parsing (13).
- **Runtime pass owed:** the keybinding gradient (7) and the sticky-error /
  worker-dead badge (6) changed behavior verified by reading, not by driving
  the TUI — worth a live smoke test.

## Open decisions (settle before touching them)

- **Keybinding gradient** (Batch 7) — changes today's muscle memory
  (`Alt+`-for-everything). Confirm before implementing.
- **Decompose `App`** (Batch 8) — how far: sub-structs vs a full per-screen
  state machine.
- **Command palette** (Batch 14) — ship it as a new feature, yes/no.
- **`#![forbid(unsafe_code)]`** — adopt crate-wide in the rewrite, yes/no.
