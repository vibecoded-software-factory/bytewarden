# AGENTS.md

<!-- Keep this file under 200 lines. Every rule must be verifiable against the
code, a test or a command. Move procedures out. Rules only, never a map of the
code: a stale map is worse than none, and `src/` cannot go out of date. -->

`bytewarden` is a terminal UI that shells out to the **Bitwarden CLI** (`bw`)
and parses its JSON: **no** Bitwarden SDK, and Bitwarden owns all cryptography.

## Never fake (hard rule, above everything else)

- Never call anything **done / complete / working / tested / at parity** unless
  you have *just checked*. Report progress as progress.
- A completeness claim about a checklist-shaped goal comes **with the
  verification inline**: enumerate the full set, grep for each, paste the diff.
  A non-empty diff means not done; say what is missing.
- Never silently drop an item. If one seems unnecessary or needs the user's
  decision, **say so and ask**.
- Unrun, unchecked or unsure: say so plainly. Never invent a value you could
  look up (codepoint, flag, API shape). The user must never be the one who
  discovers a claim was false.

## Commands and gates

```sh
cargo run                                   # needs `bw` on PATH
cargo build --release
cargo fmt --all                             # formatter of record; never hand-format
cargo clippy --all-targets -- -D warnings   # hard gate: a warning is a failure
cargo test                                  # hard gate
```

- **Before every commit**, even a one-liner, run fmt, clippy and test. Check
  real exit codes; never pipe a gate into a grep that can mask a failure.
- `BYTEWARDEN_DEBUG=1` logs redacted commands to `~/.bytewarden.log`;
  `BYTEWARDEN_GLYPHS` / `BYTEWARDEN_KEYS` override capability/keyboard detection.

## Architecture (non-negotiable)

```
main ──► tui ──► flows ──► ports ◄── adapters
                    ▲
                    └── domain (pure types, no I/O)
```

- **`domain/` performs no I/O** (no path, no `Command`, no clock), so it is
  tested without spawning a process or touching a file.
- **`ports/` are traits; `adapters/` are the only layer that does I/O.** A new
  backend implements a trait; the layers above don't change.
- **`tui/` is a driving adapter** using ports through trait objects (testable
  against fakes). Every layer may depend on `domain`; nothing depends on `tui`
  except `main`, the composition root that wires the concrete adapters.
- **Errors at the seam are typed** (`Result<_, BwError>`, never
  `Result<_, String>`), classified at the adapter boundary where the failure is
  knowable, so the UI never matches on text. A login challenge is **not** an
  error but a successful-but-incomplete outcome.

## Execution model: a worker thread and a channel

**No `tokio`, no `async-std`.** Each `bw` call is a blocking subprocess on a
worker thread that owns the vault port, reached over `mpsc`. Extend the pattern
(request/response variants, ticket, request/handle pair); never replace it.

- **One request in flight**: the ticket is an `Option`, not a queue. Input is
  gated while it is claimed, and **the mouse obeys the same gate**.
- **Claim through the shared helper** (`app.submit(..)`), never by assigning
  the ticket (`app.in_flight = Some(..)`): it stamps the watchdog and refuses a
  double-send. The bare claim (`app.begin(..)`) is only for a *silent* chained
  step that must not raise its own toast; justify it in the commit.
- **Multi-step flows chain**: a response handler queues the next request. Never
  fan out concurrent requests the busy-guard would drop.
- **Failure is contained four ways**, each closing a way to wedge the UI: a
  panic in a port call becomes an error, not a dead worker; every subprocess
  has a wall-clock deadline; a watchdog releases a ticket that outlives its
  budget; the panic hook tears down the terminal only for a UI-thread panic.
- **No background lane, no push lane**: the session key is per-adapter (a
  second worker has no session) and the CLI has no realtime stream.

## State and invalidation

- **Each derived cache has exactly one rebuild path**, next to the fields it
  protects. Mutating an input without the matching rebuild is a bug.
- **Selection indexes the *filtered* view, never the raw vec.**
- **Re-anchor by id, never by index**, after any reload.

## UI system

Exactly one implementation per visual job (overlay, list, confirm, popup,
legend, empty state, text input), and its tests are the spec. A second one is a
bug (they diverge): a UI change reuses the existing one, never a bespoke
`Block`. Each rule below is guarded by a test:

- **Legibility is a hierarchy, not a fade.** Content (navigable rows too) is
  the foreground colour, never dimmed; emphasis/interaction is the accent;
  secondary text is dim; the unfocused tint is for borders; the faintest tier is
  chrome only. Focus brightens the active thing, never fades the inactive one.
- **On the Mono tier (`NO_COLOR`), focus and selection are marked by modifier,
  never by colour**: colours map to `Reset`; focus `BOLD`, selection `REVERSED`.
- **Measure layout in terminal cells** (`Span::raw(s).width()`), never
  `s.chars().count()` or `s.len()`, except for genuine char indices.
- **Never an emoji**: one `char`, two cells, breaking the one-cell alignment of
  its row. Markers come from the icon set, single-cell by construction.
- **Shortcut labels follow the host** (Apple glyphs on macOS, spelled out
  elsewhere); every surface that prints a key routes through the one rewrite.
- **Every empty state teaches** the keys that would fill it; a bare dim line is
  not acceptable. **Every overflowing region says so** with the shared cue.
- **Fit by whole segments with an ellipsis**; never clip a keybinding in half.
  Size against real content width, not a magic number.

## Keybindings: the gradient

The modifier signals weight: **bare letter** = frequent, safe action on a
focused list that doesn't type · **`Shift`** = loud tier · **`Ctrl`** = global,
the only tier every terminal reliably delivers · **`Alt`** = app-wide command,
and row actions on a typing surface · **`/`** = focus search.
**`Ctrl+C` is the only quit.**

- A destructive action may be a bare letter *because* it goes through a
  navigable confirm: the confirm is the guard, not the modifier.
- `Esc` backs out one layer at a time and **never destroys typed text**.
- **A key added or changed is synced in the same change** on every surface
  that advertises it: footer hint, help popup, command palette.

## Text-input model

Every text input is the shared line editor: UTF-8-safe cursor, readline word
ops defined in one place, zeroized on drop (any input can hold a secret). Keys
go through the shared router, rendering through the shared renderer. **Never
hand-roll cursor editing in a screen**; that duplication has grown back before.

## Bitwarden CLI adapter

All Bitwarden access is the `bw` binary as a subprocess; new functionality is a
new invocation, never an SDK crate.

- **Map every Bitwarden feature to a `bw` command up front and cite it.** A
  command or flag the adapter doesn't already use is checked with
  `bw <cmd> --help` before designing around it.
- **Nothing sensitive in `argv`** (`ps aux`, `/proc/PID/cmdline`). Master
  password, OTP and 2FA codes go by env var or stdin; the session key by
  `BW_SESSION`, never `--session`; encoded write payloads (create, edit, move,
  send) go over **stdin**: args `["create", "item"]` plus the payload on stdin,
  never `["create", "item", &encoded]`.
- **Every invocation has a wall-clock deadline** sized to the operation; no
  runner lacks one. A timeout means "fail gracefully, let the user retry".
- **Build JSON with a serializer**, never string concatenation.
- **Parse lists row by row**, so one malformed record can't drop the list.
- **Log invocations redacted**, in the UI layer (it owns the session marker);
  an adapter-issued invocation is logged only if its flow records one.

## Security and memory hygiene (each closes an exploit; keep them)

- Session key, password and code buffers, every vault payload and every text
  input are zeroized on drop. Keep the derives.
- **Lock wipes** the in-memory vault, every form and overlay, and the command
  log (logout and shutdown wipe the vault too). **Auto-lock fires on every
  unlocked screen.**
- **Reprompt** re-verifies the master password before exposing a flagged
  item's secret, with **no caching**, on the mouse path as on the keyboard.
- **Clipboard**: auto-clear wipes only bytewarden's own write still in place; a
  tool failure is reported, and a copy that cannot auto-clear says so.
- **Config and session files are written atomically, `0600` from creation**;
  the session dir must be a real, owner-only directory (no symlink) or nothing
  is written or read. No new surface writes secrets to disk (beyond the
  user-chosen export path) without an explicit ask.

## Verification and working agreements

- **Verify before declaring done**: the full gate, tests for new pure logic,
  and a regression test for each behaviour fix that fails on the old code.
- **Fix the class, not the instance**: after a targeted fix, grep for siblings
  of the pattern and fix them all. **A change to one screen is a change to
  all**: touching a shared mechanic means checking every place it is used.
- **Judge coherence and flow first**: match the app's patterns and comparable
  tools (vim, lazygit, mutt, the Bitwarden GUI); no needless mode switches,
  cursor jumps or lost input. State the reasoning when non-trivial.
- **No `//` comments in source.** Names and tests carry the meaning.

## Workflow and git

- Integration branch is **`dev`**; never commit to it directly. One change =
  one branch (`feat/` · `fix/` · `refactor/` · `chore/` · `docs/` + slug) = one
  PR against `dev`. Commit or push only when the user asks.
- **Conventional Commits**, subject ≤ 72 chars, body explains the *why*; one
  logical change per commit.
- **No AI trailers**: no AI `Co-Authored-By`, no "Generated with" footers.
- **No cross-project references** (standalone public repo): never name or cite
  a sibling project in code, commits, PR bodies or docs; every pattern is this
  app's own. Only a declared dependency is cited, by its published crate name.

## Do not touch unprompted

- The worker/channel model (no async runtimes); the ports/adapters boundary and
  the typed seam error; existing keybindings and the shared widget system.
- The hygiene discipline: zeroize, tolerant parsing, panic isolation, subprocess
  deadlines, redacted logging, nothing sensitive in argv, no secrets on disk,
  atomic writes, owner-only perms.
- The pinned Rust toolchain. No new `unsafe` without an explicit ask, and none
  in the composition root.
