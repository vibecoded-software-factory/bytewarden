# bytewarden

A terminal UI for [Bitwarden](https://bitwarden.com), built with
[Ratatui](https://ratatui.rs). It drives the official `bw` CLI — there is no
Bitwarden SDK dependency, and Bitwarden owns all cryptography.

```
    __          __                              __
   / /_  __  __/ /____ _      ______ __________/ /__  ____
  / __ \/ / / / __/ _ \ | /| / / __ `/ ___/ __  / _ \/ __ \
 / /_/ / /_/ / /_/  __/ |/ |/ / /_/ / /  / /_/ /  __/ / / /
/_.___/\__, /\__/\___/|__/|__/\__,_/_/   \__,_/\___/_/ /_/
      /____/
```

Full CRUD over every Bitwarden item type, folders, attachments, Sends,
import/export, a password generator, organisation collections, and the
HaveIBeenPwned breach check — keyboard-driven, mouse-supported, themable.

---

## Requirements

- The [Bitwarden CLI](https://bitwarden.com/help/cli/) (`bw`) on your `$PATH`.
- A [Rust toolchain](https://rustup.rs) to build it.
- A clipboard tool, in a graphical session: `wl-copy` (Wayland), `xclip` or
  `xsel` (X11), `pbcopy` (macOS). bytewarden picks whichever is actually
  installed. If a graphical session has none it says which package to install,
  rather than silently pretending the copy worked. A **headless** session (no
  `$WAYLAND_DISPLAY` / `$DISPLAY`) falls back to the OSC 52 terminal escape,
  which works over SSH and tmux in a compatible terminal — though the timed
  auto-clear is skipped there, since OSC 52 cannot read the clipboard back to
  check it still holds what we wrote.

Unix only: the session file, the permission bits and the clipboard backends
are all POSIX.

## Install

```bash
git clone https://github.com/51lv3str1/bytewarden
cd bytewarden
cargo build --release
./target/release/bytewarden
```

Or `cargo run` while iterating.

## Using it

Start it. The splash runs `bw status` and takes you to the login form, to the
unlock prompt, or straight into the vault if a session is already live.

From there **the app documents itself** — and that is the only description
which cannot drift out of date:

- **`F1`** — help, scoped to the screen you are on, and inside the vault to
  the focused panel.
- **`Ctrl+P`** — the command palette: fuzzy-search every action valid right
  here, each row showing its shortcut. It runs the very same code the
  keybinding does, so it doubles as a cheat-sheet that cannot lie.
- The bottom bar always shows the keys that matter where you are.
- **`Ctrl+C`** quits.

Keys follow a gradient, so the modifier tells you the weight of an action
before you press it: a **bare letter** acts on the focused list, **`Ctrl`** is
global, **`Alt`** is an app-wide command, **`/`** focuses search.

## Configuration

`~/.config/bytewarden/config.toml`, created on first launch. Every key is
optional, and the parser preserves anything it does not recognise, so it is
safe to hand-edit.

Everything in it is also editable from the app — the login screen's checkboxes
and the **`F10` settings overlay** (theme · security · advanced). The overlay
writes the file as you change a value, so the two never disagree, and it lists
the settings with a one-line description each. That is why they are not
enumerated here.

The file and its directory are kept owner-only (`0600` / `0700`), re-applied
after every write.

### Theme

A `[theme]` block picks one of the bundled presets by `name`, and overrides any
individual colour with a hex value. The `F10` overlay previews presets live and
saves the one you pick, so the shipped list is best browsed there.

Colours adapt to the terminal: `NO_COLOR` collapses them to grayscale, a
terminal with no truecolor hint gets each colour quantized to the nearest
xterm-256 index, and truecolor passes through untouched. The default foreground
is inherited from your terminal, so light backgrounds stay readable.

## Environment

| Variable | Effect |
|---|---|
| `BW_SESSION` | An existing unlocked session key; picked up at boot. |
| `BW_CLIENTID` / `BW_CLIENTSECRET` | Consumed by the API-key login. |
| `BYTEWARDEN_DEBUG=1` | Also append the command log to `~/.bytewarden.log` (mode `0600`), for a session longer than the in-app panel keeps. |
| `BYTEWARDEN_GLYPHS=console\|full` | Override the terminal glyph-capability detection. |
| `BYTEWARDEN_KEYS=mac\|pc` | Override the shortcut-label convention. Useful over SSH, where the machine running bytewarden and the keyboard in front of you disagree. |
| `NO_COLOR` / `COLORTERM` | Standard; see *Theme*. |

## macOS

Shortcut labels print in the host's convention — the Apple glyphs your keys are
engraved with (`⌥N`, `⌃P`) on macOS, spelled out (`Alt+N`) elsewhere.
Presentation only; the bindings are identical on every platform.

**If `Alt` does nothing:** a default Terminal.app sends a composed character
for Option instead of Meta, so the chord never reaches the application at all.
There is no failed `Alt` for bytewarden to detect and nothing it can fall back
to — Terminal.app supports neither CSI u nor `modifyOtherKeys`, and the
protocol that would report the modifier and the character together is not
available through crossterm yet. Two ways out:

- Make Option send Meta — Terminal → Settings → Profiles → Keyboard → *Use
  Option as Meta key*; iTerm2 → Profiles → Keys → *Left Option key: Esc+*.
- Or use **`Ctrl+P`**, which every terminal delivers. Inside a form or the
  generator the palette lists *that* screen's verbs — the ones that otherwise
  live only on an `Alt` chord.

## Security

- The session key, the master-password and code buffers, every vault payload
  and every text input are zeroized: their bytes are overwritten when dropped,
  not merely freed. The in-memory vault is wiped on lock, on logout and on
  exit.
- **Secrets never reach a command line.** Passwords and codes travel by
  environment variable or stdin, the session key by `BW_SESSION` — never
  `argv`, so `ps` and `/proc` never see them.
- The command log and the debug file redact the session key.
- **Reprompt** is honoured: an item flagged for it re-asks for the master
  password before exposing a secret, every time, with no caching — on the
  mouse path as well as the keyboard.
- A copied secret is wiped from the clipboard after a configurable delay, and
  only if the clipboard still holds what bytewarden put there.
- Optional auto-lock after inactivity, and an optional per-terminal session
  file (`0600`, tied to the parent shell's lifetime, dropped when it dies).

Exports are the deliberate exception: an unencrypted export contains plaintext
credentials, by design. bytewarden refuses to overwrite an existing file so it
cannot clobber one by surprise, but where you put it is your call.

## Contributing

`CLAUDE.md` holds the working agreements — the architecture boundaries, the
invariants that must not regress, and the checks that gate a commit.

```bash
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

All three must pass; CI runs the same on every push and PR.

## License

See [`LICENSE`](LICENSE).
