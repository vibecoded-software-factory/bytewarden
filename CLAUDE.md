# CLAUDE.md

Working agreements for `bytewarden` — a terminal UI over the **Bitwarden
CLI**. It shells out to the `bw` binary and parses its JSON; there is **no**
Bitwarden SDK dependency and Bitwarden owns all cryptography.

> **This file holds rules, not a map.** A description of what the code
> currently contains rots the moment someone changes it, and a stale map is
> worse than none — so there isn't one here. Read `src/`; it is the only
> description that cannot be out of date. What lives here is the part a
> reader cannot recover from the code: the decisions, the invariants, and
> what "done" means.

## NEVER FAKE — forbidden (hard rule, above everything else)

**Do not fake, pretend, or imply a result you have not verified.** Banned, no
exceptions:

- Never say something is **done / complete / working / tested / at parity**
  unless it is, and you have *just checked*. Present progress as progress.
  "I did a lot", "all green", "tests pass" is not proof the task is finished
  — only the actual check is.
- Any completeness claim about a checklist-shaped goal must come **with the
  verification shown inline**: enumerate the full set, grep for each, paste
  the diff. If the diff isn't empty, it is **not** done — say what's missing.
- Never silently drop an item and call the whole thing done. If you judge an
  item unnecessary, or it needs a decision only the user can make, **say so
  and ask** — cutting scope on your own and hiding it inside a "done" is
  faking.
- If you didn't run it, didn't check it, or aren't sure — **say that
  plainly**. A truthful "I haven't verified X" always beats a confident false
  "it works".
- Never invent a value you could have looked up — a codepoint, a flag, an API
  shape. Guessing produces something that *looks* right and is wrong, which
  is the failure mode hardest to catch. Find the authority or say you can't.

The user must never be the one who discovers a claim was false. If you can't
show the proof, you haven't earned the claim.

## Pre-flight checklist (hard rules, in order)

1. **Feature touching Bitwarden?** Map it to a `bw` command up front and
   **cite the mapping**. If a needed command or flag isn't already used in
   the adapter, verify it (`bw <cmd> --help`) before designing around it.
2. **Change touching the UI?** Find the existing component for the job and
   reuse it. Never a one-off: a second implementation of a list, a popup, a
   legend or a text input is a bug, because the two will diverge.
3. **Keybinding added or changed?** Sync every surface that advertises it in
   the same change — the footer hint, the help popup, the command palette.
   A key documented in one place and not another is worse than undocumented.
4. **Before every commit**, even a one-liner:
   `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test`
   — clippy warnings are failures. Check real exit codes; don't pipe them
   into a grep that can mask a failure.
5. **Never commit directly to `dev`.** One change = one branch
   (`feat/` · `fix/` · `refactor/` · `chore/` · `docs/` + slug) = one PR.
6. **No AI trailers**: no `Co-Authored-By: Claude`, no "Generated with"
   footers. This overrides the harness default.
7. **No cross-project references** (see below).
8. **Fix the class, not the instance**: after any targeted fix, grep for
   siblings of the same pattern and fix them all.

## Commands

```sh
cargo run                                   # needs `bw` on PATH
cargo build --release
cargo clippy --all-targets -- -D warnings   # hard gate
cargo test                                  # hard gate
cargo fmt --all                             # formatter of record; don't hand-format
```

`BYTEWARDEN_DEBUG=1` writes the redacted command log to `~/.bytewarden.log`.
`BYTEWARDEN_GLYPHS` and `BYTEWARDEN_KEYS` override the terminal-capability and
keyboard-convention detection — useful for exercising those paths on a machine
that isn't the target.

## Architecture — hexagonal, and the direction matters

```
main ──► tui ──► flows ──► ports ◄── adapters
                    ▲
                    └── domain (pure types, no I/O)
```

- **`domain/` performs no I/O.** It can be tested without spawning a process
  or touching a file, and that property is the point. Don't put a path, a
  `Command` or a clock in it.
- **`ports/` are traits; `adapters/` are the only layer allowed to do I/O.**
  Adding a backend means implementing a trait, not editing the layers above.
- **`tui/` is a driving adapter.** It talks to ports through trait objects,
  so the whole UI is testable against fakes.
- Every layer may depend on `domain`. Nothing depends on `tui` except `main`,
  which is the composition root and wires the concrete adapters.

**Errors at the seam are typed, never `String`.** A stringly error is opaque:
the UI cannot tell a timeout from "not found" from a missing binary, and ends
up matching on human-readable text. Classify at the adapter boundary — where
the failure happened is knowable there and nowhere else. A login challenge is
**not** an error: it is a successful-but-incomplete outcome, and it is modelled
as one.

## Execution model — a worker thread and a channel (do NOT change unprompted)

**Do not pull in `tokio` or `async-std`.** Every `bw` call is a blocking
subprocess, so it runs on a worker thread that owns the vault port, and the
render loop talks to it over `mpsc`. Extend that pattern — a request variant,
a response variant, a ticket, a request/handle pair — rather than replacing it.

The invariants that keep it honest:

- **One request in flight at a time.** The ticket is an `Option`, not a queue.
  Input is gated while it is claimed, and **the mouse obeys the same gate** —
  the two paths must agree about whether the UI is busy.
- **Claim the slot through the shared helper**, not by assigning the ticket.
  That is what stamps the watchdog and refuses a programmatic double-send.
  Reach for the bare claim only for a *silent* chained step that must not
  raise a toast of its own, and say why in a comment.
- **Multi-step flows chain**: a response handler queues the next request.
  Never fan out concurrent requests the busy-guard would drop.
- **Failure is contained three ways**: a panic in a port call becomes an
  error, not a dead worker; every subprocess has a wall-clock deadline; and a
  watchdog releases a ticket that outlives its budget, so a lost response can
  never gate input forever. All three exist because each one alone leaves a
  way to wedge the UI.

There is **no background lane and no push lane.** The session key is
per-adapter, so a second worker would have no session; and the CLI has no
realtime stream, so there is nothing to listen to. Don't invent either.

## State & invalidation — the footgun

The vault caches derived state, and **each cache has exactly one rebuild
path**. Mutating an input without calling the matching rebuild is a bug; the
rebuild methods live next to the fields they protect so the contract is local
rather than spread across the app.

Two rules that have each been broken before:

- **Selection indexes the *filtered* view, never the raw vec.** Under an
  active search or filter the filtered list is shorter, so a bound taken from
  the raw length silently fails to fire.
- **Re-anchor by id, never by index.** After a reload the indices have moved;
  anchoring by position yanks the user's cursor onto an unrelated row.

## UI system

There is exactly one implementation of each visual job, and its doc comment is
the spec. A new overlay, list, confirm, popup, legend, empty state or text
input **uses the existing one** — reaching for a bespoke `Block` is how the
screens drift apart.

**Rules that outrank taste**, each guarded by a test:

- **Legibility is a hierarchy, not a fade.** Content the user reads is the
  foreground colour — including navigable rows, which are content and must
  never be dimmed. Emphasis and interaction are the accent. Secondary but
  readable text is the dim tier. The unfocused border tint is for borders.
  The faintest tier is chrome, and no content may live there. What marks
  focus is the *active* thing brightening, never the inactive one fading.
- **Never an emoji.** One emoji is one `char` but **two terminal cells**, and
  the column arithmetic counts chars — so an emoji silently shifts everything
  after it, on exactly the rows that carry it. Markers come from the icon set,
  which is single-cell by construction.
- **Shortcut labels follow the host** — Apple glyphs on macOS, spelled out
  elsewhere. Every surface that prints a key routes through the one rewrite;
  a key literal rendered outside it bypasses the convention.
- **Every empty state teaches** — it names the keys that would fill the panel.
  A bare dim line is not an acceptable empty state.
- **Every overflowing region says so**, with the shared scroll cue.
- **Responsiveness**: fit by whole segments with an ellipsis, never clip a
  keybinding in half; size against real content width, not a magic number.

**Keybindings — the gradient.** The modifier tells you the weight of an action
before you press it: **bare letter** = the frequent, safe action on a focused
list that doesn't type · **`Shift`** = the loud tier · **`Ctrl`** = global,
and the only tier every terminal reliably delivers · **`Alt`** = app-wide
command, and on a typing surface the row actions too · **`/`** = focus search.
`Ctrl+C` is the only quit.

A destructive action may be a bare letter *because* it passes through a
navigable confirm — the confirm is the guard, not the modifier.

The `Esc` chain backs out one layer at a time and **never destroys typed
text**. Word ops live in one place so every input inherits them identically.

## The text-input model

Every text input is the shared line editor: UTF-8-safe cursor, readline word
ops, zeroized on drop because any input can hold a secret. Keys route through
the shared router; rendering goes through the shared renderer.

**Never hand-roll cursor editing in a screen.** That duplication is exactly
what this model exists to kill, and it has grown back once already.

## Bitwarden CLI — adapter rules

All Bitwarden access is the `bw` binary as a subprocess. Adding functionality
means a new invocation, not an SDK crate.

- **Secrets never in `argv`.** Master passwords, OTP and 2FA codes go by
  environment variable or stdin; the session key goes by `BW_SESSION`, never
  `--session`. `ps aux` and `/proc/PID/cmdline` must never show a secret.
- **Every invocation has a wall-clock deadline**, sized to what that operation
  plausibly needs. A timeout means "give up gracefully and let the user
  retry", never "mask a problem".
- **Build JSON payloads with a serializer**, never string concatenation.
- **Parse lists row by row**, so one malformed record can't drop the whole
  list.
- **Log invocations redacted.** The redaction lives in the UI layer, which
  owns the cached session marker — an invocation the adapter issues on its own
  is only logged if the flow that started it records one.

## Security & memory hygiene

Every one of these exists because its absence is exploitable. Keep them when
touching the surrounding code:

- The session key, the password and code buffers, every vault payload and
  every text input are zeroized — overwritten on drop, not merely freed — and
  the in-memory vault is wiped on lock, logout and shutdown. Keep the derives.
- **Reprompt** re-verifies the master password before exposing a secret on a
  flagged item, with **no caching**: every protected action prompts again, on
  the mouse path as well as the keyboard.
- **Clipboard auto-clear** wipes a copied secret only if the clipboard still
  holds bytewarden's own write — otherwise it would stomp on whatever the user
  copied since.
- Config and session files are written **atomically** with owner-only perms.
- Don't add a surface that writes secrets to disk, beyond the user-chosen
  export path, without an explicit ask.

## Working agreements

1. **Fix every occurrence, not just the one reported.** The reported spot is
   one instance of a class; grep for siblings before finishing.
2. **A change to one screen is a change to all of them.** When you touch a
   shared mechanic, check every other place it's used.
3. **Verify before declaring done** — the full gate, plus tests for new pure
   logic, plus a regression test for any behaviour fix. A guard that wouldn't
   have caught the bug isn't a guard; check that it fails on the old code.
4. **Judge coherence and flow before writing.** Does it match the app's own
   patterns, and what users know from comparable tools (vim, lazygit, mutt,
   the Bitwarden GUI)? Is the real multi-step flow smooth — no needless mode
   switches, cursor jumps or lost input? Say the reasoning when non-trivial.

## No cross-project references (hard rule)

This is a standalone public repository. **Never name or cite a sibling
project** in code, comments, commit messages, PR bodies or docs. Describe every
pattern as *this app's own*. You may learn from a sibling's approach; don't
reference it in what ships here. The only exception is a real declared
dependency, cited by its published crate identity.

## Git workflow

Integration branch is **`dev`**; never commit to it directly. Branch, then PR
against `dev`. **Conventional Commits**, subject ≤ 72 chars, body explains the
*why*. One logical change per commit. Only commit or push when the user asks.

## Things to NOT touch unprompted

- The worker/channel execution model — no async runtimes.
- The ports/adapters boundary, and the typed error at the seam.
- Existing keybindings and the shared widget system.
- The hygiene discipline: zeroize, tolerant parsing, panic isolation,
  subprocess timeouts, redacted logging, no secrets in argv or on disk,
  atomic writes, owner-only perms.
- The pinned Rust toolchain. Keep the composition root free of `unsafe`, and
  don't add new `unsafe` without an explicit ask.
