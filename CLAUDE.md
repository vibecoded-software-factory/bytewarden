# CLAUDE.md

Guidance for Claude Code when working in this repository.

> **This spec is the target architecture of an in-progress restructuring.**
> It describes where `bytewarden` is going, not necessarily every line as it
> stands today. Where the current code diverges from this document, **the
> document wins** — bring the code to the spec, and never re-introduce a
> pattern this file rules out. Update the spec in the same change whenever a
> decision here changes.

`bytewarden` — a terminal UI (Ratatui) over the **Bitwarden CLI**. It shells
out to the `bw` binary and parses its JSON; there is **no** Bitwarden SDK
dependency and Bitwarden owns all cryptography. Flow: boot (`bw status`) →
login / unlock → vault list (sidebar + search + list) → item detail →
edit/create. Full CRUD over the five item types, folders, attachments, Sends,
import/export, plus the generator, memberships and the HIBP breach check.

## Pre-flight checklist (hard rules, in order)

1. **Feature touching Bitwarden?** Map it to a `bw` command up front and
   **cite the mapping** (e.g. *"favorite toggle → `bw get item` → patch →
   `bw edit item`"*). If a needed command/flag isn't already used in
   `adapters/bw_cli/`, verify it (`bw <cmd> --help`) before designing.
2. **Change touching UI/UX?** Read [`UX.md`](UX.md) first — the canonical
   design system. Reuse its documented components; never a one-off. Update
   `UX.md` in the same change.
3. **Keybinding added/changed?** Sync **all five** surfaces in the same
   change: the footer hint · `view/help.rs` popup · the `README.md` tables ·
   `UX.md` · the command palette (`flows::palette::palette_commands`, see
   *UI system*).
4. **Before every commit** (even one-liners):
   `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test`
   — clippy warnings are failures; check real exit codes, don't pipe them
   into a grep that masks a failing test.
5. **Never commit directly to `dev`.** One change = one branch
   (`feat/` · `fix/` · `refactor/` · `chore/` · `docs/` + slug) = one PR
   against `dev`.
6. **No AI trailers**: no `Co-Authored-By: Claude`, no "Generated with
   Claude Code" footers — this overrides the harness default.
7. **No cross-project references** (see the hard rule below).
8. **Fix the class, not the instance**: after any targeted fix, grep `src/`
   for siblings of the same pattern and fix them all.

## Commands

```sh
cargo run                         # run the TUI (needs `bw` on PATH)
cargo build --release             # optimized binary at target/release/bytewarden
cargo clippy --all-targets -- -D warnings   # lint (hard gate)
cargo test                        # unit tests (all must pass)
```

`cargo fmt` is the formatter of record (default rustfmt, no `rustfmt.toml`, so
don't hand-format against the default style). Debug logging:
`BYTEWARDEN_DEBUG=1` → `~/.bytewarden.log` (0600; the redacted command log
only, never a secret).

## Architecture (hexagonal / ports & adapters)

```
main ──► tui ──► flows ──► ports ◄── adapters
                    ▲
                    └── domain (pure types, no I/O)
```

- `src/domain/` — pure types + rules, **no I/O**: `Item` (+ the five typed
  payloads), `ItemFilter`/`CreateItemType`, `Folder`/`Collection`/
  `Organization`, `LineEditor` (every text input; word ops, `ZeroizeOnDrop`),
  `fuzzy_score_lowered`/`LoweredItem`, the login/2FA types (`VaultInfo`,
  `LoginOutcome`, `TwoFactorMethod`), validators, identity helpers.
- `src/ports/` — traits: `VaultPort`, `ClipboardPort`, `SettingsPort`,
  `PasswordGeneratorPort` (+ the typed `BwError`). Every port method returns
  `Result<_, BwError>`, never `Result<_, String>` (see *Error taxonomy*).
  `SettingsPort::write_*` return `bool` — never fatal, but callers must
  inform on failure.
- `src/adapters/` — the **only** layer doing I/O:
  - `bw_cli/` — subprocess + `codec` (serde-built JSON payloads, base64) +
    `process` (wall-clock timeouts, concurrent piped-drain) + `json`
    (tolerant parsing). One-shot subprocess model; there is **no** persistent
    session or push stream (the `bw` CLI has neither — see *Execution model*).
  - `clipboard_system.rs` — wl-copy/xclip/xsel/pbcopy + **OSC 52** fallback
    for headless, with compare-and-clear auto-clear (`clipboard_clear_secs`).
  - `settings_toml.rs` — hand-rolled TOML preserving unknown keys, **atomic**
    writes (temp file + `rename`), 0700/0600 perms.
  - `bw_generator.rs` — the password/passphrase generator (`bw generate`).
- `src/tui/` — the driving adapter:
  - `app.rs` — the mutable `App` state container. Per-screen state lives in
    sub-structs (`vault::Vault` for the item list + its invalidation
    contract, `login_form`, `item_forms`, `settings_overlay`, the generator
    and the popup states); `App` keeps navigation, session reference data,
    worker plumbing and the injected ports.
  - `worker.rs` — worker thread(s) + `WorkerRequest`/`WorkerResponse`/
    `InFlight`; `run_caught` panic isolation per call.
  - `flows/` — per-feature `request_*`/`handle_*` pairs (`auth`, `vault`,
    `items`, `copy`, `generator`, `folders`, `memberships`, `reprompt`,
    `export`, `import`, `send`, `assign_collections`, `item_json`).
    `flows::apply_response` routes each response by the `in_flight` ticket.
  - `input/` — per-screen key handlers (router `input/mod.rs`) + `mouse.rs`;
    shared mechanics in `input/common.rs` (`list_nav`, `route_line_editor`,
    `search_key`, `confirm_key` + `run_confirm`, `busy_blocks`,
    `cycle_focus`) — every handler delegates, never re-implements.
  - `view/` — per-screen renderers (router `view/mod.rs::draw`; popups draw
    their base screen underneath) + the widget system in `view/widgets.rs`
    (see *UI system*); `logo.rs`/`starfield.rs` (splash/login only).
  - Support modules under `tui/`: `theme.rs` (presets + `ColorCaps`
    adaptation + semantic styles), `settings_model.rs`, `action.rs`
    (`ActionState`/`CmdEntry`), `screens.rs` (`Screen`/`Focus`),
    `mouse_areas.rs` (hit-test rects), `session_file.rs` (keep-session
    per-PPID file), `debug_log.rs`.

New screen = `Screen` variant + `input/<screen>.rs` (wired in the router) +
`view/<screen>.rs` (wired in `draw`) + a `view/help.rs` section + the
four-surface keybinding sync. **Reuse the shared helpers rather than
re-implementing per screen** — that is what keeps the app coherent.

## Execution model — worker thread(s) + mpsc (do NOT touch unprompted)

**Do NOT pull in `tokio`/`async-std`.** Extend the `std::thread` + `mpsc`
pattern instead: a `WorkerRequest`/`WorkerResponse`/`InFlight` variant + a
`request_*`/`handle_*` pair.

- **User lane** — one worker owns the `VaultPort` (+ generator) and serves
  requests serially. A flow starts a request with
  **`App::submit(slot, label, req)`**: it claims the `in_flight` slot via
  `begin`, shows the `Running` toast, and sends; a failed send releases the
  slot and routes through `on_worker_dead` instead of leaving the UI busy.
  Only reach for bare `begin()` when state must mutate between claiming and
  sending — comment why. **One request in flight at a time**
  (`App::in_flight: Option<_>`); `input::common::busy_blocks` gates every key
  but `Esc` while busy so a second request can't be queued.
- **Background lane** (optional, for silent work that must never gate the
  user): the post-mutation silent reloads (reload-items, reload-trash,
  reload-folders after a delete/import/move) and the auto-lock-safe idle
  resync belong here. Its responses carry **no ticket** and are routed **by
  variant** at the top of `apply_response`, off the user's `in_flight` slot.
- **No push lane.** The `bw` CLI has no realtime stream, so there is no
  `api-listen` equivalent and no listener supervisor. The vault is refreshed
  by explicit sync / reload, not by pushed events. Do not invent one.
- **Multi-step flows chain** by having a `handle_*` queue the next
  `request_*`: login → load items → parallel session-data; save = fetch item
  JSON → patch → `edit`; favorite = fetch → flip → `edit`; delete → reload
  trash; folder-delete → reload items → reload folders; import → reload items
  → reload folders. Prefer a single explicit chain per operation; never fan
  out N concurrent requests the busy-guard would drop — sequence them as a
  batch advanced by each response.
- **Failure containment** — `run_caught` wraps every port call in
  `catch_unwind` (a panic becomes `BwError::Internal`, the worker keeps
  serving); per-op wall-clock timeouts in `process.rs`; the all-workers-dead
  state is observable (`TryRecvError::Disconnected` → `App::on_worker_dead`
  unwedges the UI, `begin` refuses, a persistent error badge shows) plus a
  per-tick **watchdog** (`App::watchdog_release_stuck_request`) that releases
  a slot outliving the largest per-op budget so a lost ticket can't lock
  input forever.

Clipboard + settings stay **synchronous** on the render thread (they're fast).
Plain clipboard copies (username/password) read an in-memory item and don't
touch the worker; only TOTP / HIBP do.

## Error taxonomy — `BwError` (do NOT return `String`)

Every port method returns `Result<_, BwError>`. Stringly-typed errors are
opaque — the UI can't tell a timeout from "not found" from a missing binary,
and ends up string-matching human-readable output. Classify at the boundary
instead:

- `Spawn(String)` — couldn't exec `bw` (not on PATH / perms).
- `Timeout { label, secs }` — wall-clock budget exceeded, child killed.
- `Exit { stderr, status }` — non-zero exit; stderr passed through verbatim.
- `InvalidJson { detail }` — stdout wasn't the JSON we expected.
- `Shape(String)` — parsed, exit 0, but an expected field/shape was missing.
- `Internal(String)` — an adapter/worker panic captured via `catch_unwind`,
  or an internal precondition failure (e.g. a session-required call while
  the vault is locked).

Login challenges (a device-verification OTP, a permanent 2FA code) are **not**
a `BwError` — they're a successful-but-incomplete outcome modelled by the
domain `LoginOutcome` (`NeedsDeviceVerification` / `NeedsTwoFactor`). The
brittle prompt-string classification stays isolated in the adapter
(`combined_outcome`); a future batch may lift it into a dedicated `Auth`
variant if `bw` ever exposes a structured signal.

`BwError` implements `Display` (human-readable, for the toast + command log)
and `std::error::Error`. The command log stores the classified error; the
feedback strip renders `Display`.

## State & invalidation contracts (the footgun list)

The **`Vault`** sub-struct (`tui/vault.rs`, reached as `app.vault`) caches
derived state; each cache has **exactly one** rebuild path, and the rebuild
methods live on `Vault` beside the fields they protect (so the contract is
local, not spread across the app). **Mutating the input without calling the
rebuild is a bug**, and calling a rebuild with the wrong cursor semantics is a
UX regression. Selection always indexes the **filtered** cache, never the raw
vec, and is re-anchored by **id**, never by index, after a wholesale reload.
All calls below are methods on `app.vault`.

| Input mutated | Must call | Notes |
|---|---|---|
| `items` / `trashed_items` replaced wholesale (load, sync, import) | `rebuild_caches()` | rebuilds lowered projection + filtered cache + sidebar counts, in that order (filtered references the lowered vec) |
| in-place edit of a searchable field (name/username/uri/notes) | `rebuild_caches()` | the lowered projection is now stale |
| item added / removed (create, delete, restore) | `rebuild_caches()` | indices in `filtered_cache` shift |
| favorite / folder / collection change only | `rebuild_filtered_cache()` + `rebuild_sidebar_counts()` | no lowered rebuild needed — names/labels unchanged |
| new search / filter / folder-filter query | `rebuild_filtered_cache()` | snaps `selected_index` to the first (top-ranked) match |
| theme / settings change | (no cache) | re-resolve the theme; nothing to invalidate |

After any load/reload handler, re-anchor `selected_index` onto the same item
**by id** so a background resync never yanks the cursor. `selected_index`
must only ever reach `items`/`trashed_items` through the filtered cache via
`.get()`, so it can never point out of bounds.

## UI system

**Read `UX.md` before any UI change** — it is the spec; keep it updated in
the same change. The component vocabulary lives in `view/widgets.rs` (a new
overlay / hint / empty-state / input **must** use these, never a one-off):

- `titled_block` / `rounded_block` / `focus_style` — the rounded panel chrome
  and the single focused-vs-unfocused style decision.
- `list_table` — every multi-column list panel (never a stretching `Min` on a
  non-final column). `draw_picker_modal` — every centered query/list overlay.
  `draw_input_popup` — small single-input popups.
- `draw_confirm_popup` + `input::common::run_confirm` — every y/n confirm
  (navigable, **default = cancel** on destructive).
- `legend_line(&[(key, label)], width, theme)` — every hint/legend (keys in
  accent via `key_style`, fitted by whole segments, never a clipped key).
- `editor_spans` / `editor_spans_masked` / `editor_lines` — the one
  text-input renderer, over a `domain::LineEditor` (see below).
- `empty_state_lines(head, hints, theme)` — every empty state **teaches**
  (names the 2-3 keys that would fill the panel); a bare dim line is not
  acceptable. `draw_scrollbar` — one scrollbar on every overflowing region.
- `unread`/attention emphasis, `favorite_star`, `key_style`, `center_rect`,
  `MODAL_*` — one definition each; `Theme::emphasis()` / `Theme::danger_title()`
  are the semantic styles.

**Keybindings — the gradient (full spec in `UX.md`).** Keys are assigned by a
gradient of modifier tiers so the modifier tells you the weight of the action
before you press it: **bare letter** = the frequent/safe action on the focused
list · **`Shift`** = the loud/status-change tier · **`Ctrl`** = global
(works from any focus) · **`Alt`** = jump to a panel + compose-context verbs ·
**`/`** = focus search. Destructive item ops are bare (`x`/`Alt+D`) *because*
they pass through the navigable confirm. `Ctrl+C` is the **only** quit. The
vim layer is a contract: the `Esc` chain backs out one layer at a time and
**never destroys typed text**; word ops (`Ctrl+W`, `Ctrl+U`, `Ctrl+←/→`,
`Ctrl+A/E`) live once in `route_line_editor` so every input inherits them.

## The text-input model

Every text input is a `domain::LineEditor` (UTF-8-safe char-index cursor;
`insert`/`backspace`/`delete`/`left`/`right`/`home`/`end`/`set`/
`clear` + readline word ops, `ZeroizeOnDrop` because inputs can hold secrets).
Handlers feed keys through `input::common::route_line_editor` (returns `true`
when the text changed → rebuild a filter) or, for the search box,
`input::common::search_key`. Rendering is always `widgets::editor_spans`.
**Do not** hand-roll `char_indices().nth()` cursor editing in a screen — that
duplication is exactly what this model exists to kill. Login/password/OTP
fields are `LineEditor`s too (masked via `editor_spans_masked`).

## Bitwarden CLI — adapter rules

All Bitwarden access is the `bw` binary spawned as a subprocess
(`adapters/bw_cli/`). Adding functionality means a new CLI invocation, not an
SDK crate:

- Build argv/JSON payloads in `codec.rs` (serde → base64, **never** string
  concat), run with a per-op **timeout** via `process.rs`, parse
  **strict-but-tolerant** in `json.rs` (skip a malformed row with a
  diagnostic, never drop the whole list).
- **Secrets never in argv.** Master passwords / OTP / 2FA codes are fed via
  stdin or the `BW_PASS_INPUT` env var; the session key via `BW_SESSION`,
  never `--session`. (`ps aux` / `/proc/PID/cmdline` must never show a
  secret.)
- Every invocation is appended to the in-app command log with the session key
  **redacted** (`***`); the same redacted line goes to `~/.bytewarden.log`
  under `BYTEWARDEN_DEBUG=1`.
- `parallel_session_data` overlaps the four post-login reads
  (folders/orgs/collections/import-formats) by cloning the adapter (sharing
  one `Arc<Zeroizing>` session key) across short-lived threads; a partial
  failure surfaces per-result, never poisons the whole load.

## Working agreements

1. **Fix every occurrence, not just the one reported** — the reported spot is
   one instance of a class; grep for siblings before finishing.
2. **Every UX change stays coherent with the whole UI** — reuse the
   documented component; when touching a shared mechanic (the confirm
   mechanics, the Esc layering, the filter contract), check every other place
   it's used.
3. **Verify before declaring done** — the full gate (fmt/clippy/test) plus
   unit tests for new pure logic on `App`/`domain`. Regression tests
   accompany behaviour fixes.
4. **Judge coherence + flow BEFORE writing.** Does the change match the app's
   own patterns; does it match what users know from comparable tools (vim /
   lazygit / mutt and the Bitwarden GUI); is the real multi-step flow smooth
   (no needless mode switches, cursor jumps, or lost input)? State the
   reasoning briefly when non-trivial.

## Security & memory hygiene

- The session key, master-password buffer, OTP/2FA buffer, every vault
  payload (`Item` & friends), and every `LineEditor` are `Zeroizing` /
  `ZeroizeOnDrop`; the in-memory cache is wiped on lock / logout / shutdown.
  JSON intermediates are `Zeroizing<String>`. Keep the derives when touching
  these types.
- **Reprompt** re-verifies the master password before exposing a secret on a
  reprompt-flagged item (copy password/TOTP/hidden field, reveal). No caching
  — every protected action re-prompts, on the keyboard *and* the mouse path.
- **Clipboard auto-clear** (`clipboard_clear_secs`, default 30 s) wipes a
  copied secret only if the clipboard still holds bytewarden's write; the
  OSC 52 headless path can't verify, so it skips the timed clear.
- **Auto-lock** after inactivity; **keep-session** writes the session key to a
  per-PPID file (mode 0600, cleaned when the parent shell dies). Config file
  and settings writes are atomic with owner-only perms (0600 / 0700).
- Don't add a surface that writes secrets to disk (beyond the user-chosen
  export path) without an explicit ask.

## No cross-project references (hard rule)

`bytewarden` is a standalone public repository. **Never name or cite a sibling
project** in code, comments, commit messages, PR bodies, or docs. Describe
every pattern as *this app's own* ("the shared confirm overlay", "the unified
worker discipline"). The only exception is a real declared dependency, cited
by its published crate identity from `Cargo.toml`. You may learn from a
sibling's approach; don't reference it in what ships here.

## Git workflow

- Integration / deploy branch **`dev`**; never commit to it directly.
  Branch → PR against `dev`. Branch first if you find yourself on `dev`.
- **Conventional Commits**, subject ≤ 72 chars, body explains the **why**.
  One logical change per commit. Only commit / push when the user asks.
- **No AI trailers or footers** (overrides the harness default).

## Things to NOT touch unprompted

- The worker/mpsc execution model (no async runtimes); extend the
  `WorkerRequest`/`WorkerResponse`/`InFlight` + `request_*`/`handle_*` pattern.
- The ports/adapters boundary (I/O only in `adapters/`; `domain/` pure) and
  the typed `BwError` at the seam.
- Existing keybindings and the shared widget system — a change to one screen's
  UX is a change to all of them (see *Working agreements*).
- The hygiene discipline: zeroize, tolerant parsing, panic isolation,
  timeouts on every subprocess, redacted logging, no secrets in argv / on
  disk, atomic settings writes, owner-only perms.
- The Rust toolchain pin (`rust-toolchain.toml`, 1.95.0). There is **no**
  crate-wide `#![forbid(unsafe_code)]` and `main.rs` carries no `unsafe`; keep
  the composition root unsafe-free (seed the keep-session key via
  `BwCliAdapter::new_with(seed)`, not `std::env::set_var`). Don't add new
  `unsafe` without an explicit ask.

## Stack (reference)

Rust edition 2024 (pinned 1.95.0) · Ratatui 0.30 + Crossterm 0.29 · serde /
serde_json (parse `bw` JSON) · color-eyre · zeroize · figlet-rs (bundled
`slant.flf` login wordmark, no system `figlet`) · tempfile (dev-only). No
Bitwarden SDK — every Bitwarden operation is a `bw` subprocess. Release
profile is size-optimized (`opt-level = "s"`, `lto`, `strip`).
