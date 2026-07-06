# bytewarden

A terminal UI for [Bitwarden](https://bitwarden.com), built with [Ratatui](https://ratatui.rs).
Wraps the official `bw` CLI — keyboard-driven, mouse-supported vault browser
with full CRUD over items, folders, attachments, sends, exports and more.

```
    __          __                              __
   / /_  __  __/ /____ _      ______ __________/ /__  ____
  / __ \/ / / / __/ _ \ | /| / / __ `/ ___/ __  / _ \/ __ \
 / /_/ / /_/ / /_/  __/ |/ |/ / /_/ / /  / /_/ /  __/ / / /
/_.___/\__, /\__/\___/|__/|__/\__,_/_/   \__,_/\___/_/ /_/
      /____/
```

---

## Table of contents

- [Features](#features)
- [Requirements](#requirements)
- [Installation](#installation)
- [Quick start](#quick-start)
- [Configuration](#configuration)
- [Theme](#theme)
- [Login screen](#login-screen)
- [Session resume](#session-resume)
- [Keep session (per-terminal persistence)](#keep-session-per-terminal-persistence)
- [Vault screen](#vault-screen)
- [Item detail](#item-detail)
- [Create item](#create-item)
- [Folders](#folders)
- [Password generator](#password-generator)
- [Export / Import](#export--import)
- [Send (text)](#send-text)
- [Memberships (organisations + collections)](#memberships-organisations--collections)
- [Attachments](#attachments)
- [Mouse support](#mouse-support)
- [Help popup (F1)](#help-popup-f1)
- [Command log](#command-log)
- [Auto-lock](#auto-lock)
- [Architecture](#architecture)
- [Development](#development)
- [Testing & coverage](#testing--coverage)
- [Keyboard reference](#keyboard-reference)
- [License](#license)

---

## Features

- All five Bitwarden item types: **Login, Secure Note, Card, Identity, SSH Key**.
- Full item CRUD — create, edit (in-place form), delete (trash + permanent), restore.
- **Folders** — list, filter, create, rename, delete; "(No folder)" bucket.
- Custom fields with cycling type (text / hidden / boolean / linked) and inline rename.
- Multi-URI logins with per-URI match-detection mode.
- **Password & passphrase generator** — class flags, ambiguous-char filter, word count, separator, capitalisation.
- Live **fuzzy search** across name, username and URI.
- **Attachments** — upload, download (with destination picker) and delete.
- **Bitwarden Send** — create text Sends with expiry and auto-copy of the URL.
- **Export** (CSV / JSON / encrypted JSON) and **Import** (any `bw import --formats`).
- **HaveIBeenPwned breach check** for any login password (`Alt+X`).
- **Memberships** read-only view: organisations and their collections.
- **Three login methods**: master password, headless API key (`BW_CLIENTID/SECRET`), SSO (browser).
- **Self-hosted** support — edit Server URL on the login screen.
- Configurable **auto-lock** after inactivity.
- Optional **per-terminal session persistence** — skip the master-password prompt on relaunch as long as the parent shell is alive.
- Per-screen **scoped & scrollable help popup** (F1) — only shows shortcuts valid in the current context.
- **Mouse support** — click panels, items, fields; scroll wheel everywhere; double-click semantics.
- **Themable** — all colors driven by a single `[theme]` block in `config.toml`.
- **Command log** of every `bw` invocation, with session keys redacted.

---

## Requirements

- [Bitwarden CLI](https://bitwarden.com/help/cli/) (`bw`) installed and on `$PATH`.
- [Rust toolchain](https://rustup.rs) (`cargo`) to build from source.
- A clipboard tool: `wl-copy` (Wayland), `xclip` / `xsel` (X11), or `pbcopy` (macOS).
  Optional — with none installed, bytewarden falls back to the **OSC 52**
  terminal escape (works over SSH / tmux in a compatible terminal), though the
  timed auto-clear is skipped on that path (OSC 52 can't read the clipboard back).

The login wordmark uses the bundled `slant` FIGlet font via `figlet-rs` — no system `figlet` install needed.

### Install system dependencies

**Ubuntu / Debian**
```bash
npm install -g @bitwarden/cli   # or: snap install bw
sudo apt install wl-clipboard   # Wayland
sudo apt install xclip          # X11
```

**Arch Linux**
```bash
sudo pacman -S bitwarden-cli wl-clipboard   # Wayland
sudo pacman -S bitwarden-cli xclip          # X11
```

**macOS**
```bash
brew install bitwarden-cli
# pbcopy is built-in
```

**Rust (all platforms)**
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### Rust crate dependencies

| Crate | Version | Purpose |
|-------|---------|---------|
| `ratatui` | 0.30 | Terminal UI framework |
| `crossterm` | 0.29 | Cross-platform terminal control, keyboard & mouse |
| `serde` + `serde_json` | 1 | Parse `bw` CLI JSON output |
| `color-eyre` | 0.6 | Error reporting |
| `figlet-rs` | 0.1 | Login wordmark with bundled `slant` font |
| `tempfile` (dev) | 3 | Tempdir helper for filesystem tests |

---

## Installation

```bash
git clone https://github.com/51lv3str1/bytewarden
cd bytewarden
cargo build --release
./target/release/bytewarden
```

Or run from source while iterating:

```bash
cargo run
```

Release profile is tuned for size: `lto = true`, `codegen-units = 1`, `opt-level = "s"`, `strip = true`.

---

## Quick start

1. Start `bytewarden`.
2. The splash screen runs `bw status` and either:
   - Routes you to the **login form** if no session is active.
   - Pre-fills the email and jumps to the **password field** if your account is *Locked*.
   - Loads the vault directly if `$BW_SESSION` is exported and valid (*Unlocked*).
3. Enter your credentials. If your backend triggers a "new device" challenge, an OTP field appears — paste the code from your e-mail and hit `Enter`.
4. Once unlocked, navigate the vault with `j`/`k`, search with `/`, open an item with `Enter`, copy values with `Alt+C` / `Alt+U`, and so on. Press `F1` from any screen for context-aware help.
5. Quit with `Ctrl+C`.

---

## Configuration

Config file: `~/.config/bytewarden/config.toml`. The file is auto-created on first launch; every key is optional.

```toml
# ── Login & session ──────────────────────────────────────────────
save_email   = true                        # remember the e-mail across launches
email        = "you@example.com"           # only stored when save_email = true

# ── Security ─────────────────────────────────────────────────────
auto_lock            = false               # lock vault after inactivity
keep_session         = false               # persist BW_SESSION while parent shell is alive
lock_after_minutes   = 15                  # idle threshold for auto-lock
clipboard_clear_secs = 30                  # auto-clear copied secrets (0 = disabled)

# ── Theme (every key optional — see the next section) ────────────
[theme]
name        = "nord"                       # bundled preset (see Theme below)
accent      = "#cba6f7"                    # optional overrides on top
inactive    = "#a6adc8"
selected_bg = "#313244"
# … (see Theme below)
```

The TOML parser is forgiving: unknown keys, mis-typed values and unrecognised sections are preserved verbatim across rewrites, so you can hand-edit the file and bytewarden will not clobber your additions.

The config file and its parent directory are kept owner-only — bytewarden re-applies `chmod 0600` to `config.toml` and `0700` to the directory after every write, so even values like your e-mail address or the `keep_session` flag never become world-readable.

### Where each setting is also editable from the UI

- `save_email` and `email` — login screen "Save email" checkbox.
- `auto_lock` — login screen "Auto-lock" checkbox.
- `keep_session` — login screen "Keep session" checkbox.
- `lock_after_minutes` — only via the config file (no UI toggle).
- `clipboard_clear_secs` — only via the config file. Default `30` (seconds); set to `0` to disable. Applies to every clipboard write that carries a secret (passwords, usernames, TOTP codes, copied detail-view fields, generated values, Send URLs). The clear is contingent on the clipboard still holding the value bytewarden wrote — if you copied something else in the meantime, your selection is left alone.
- `[theme]` — only via the config file.

---

## Theme

All colors are driven by the `[theme]` block. Pick a bundled **preset** with
`name`, then override individual keys if you want. Every key is optional;
omitting `name` keeps the shared default (Nord with terminal-inherited text).

Fourteen presets ship (dark first, light last):

| `name` | palette |
|---|---|
| `nord` | Nord (default, dark) |
| `catppuccin-mocha` · `catppuccin-macchiato` · `catppuccin-frappe` | Catppuccin (dark) |
| `dracula` | Dracula (dark) |
| `tokyonight` | Tokyo Night (dark) |
| `gruvbox-dark` | Gruvbox Dark |
| `rose-pine` | Rosé Pine (dark) |
| `everforest` | Everforest (dark) |
| `one-dark` | One Dark |
| `solarized-dark` | Solarized Dark |
| `catppuccin-latte` | Catppuccin Latte (light) |
| `rose-pine-dawn` | Rosé Pine Dawn (light) |
| `solarized-light` | Solarized Light (light) |

Themes also **adapt to the terminal's color capability**: `NO_COLOR` collapses
to grayscale, a non-truecolor terminal (`COLORTERM` unset) gets each color
quantized to the nearest xterm-256 index, and truecolor passes through.

You can also switch presets live from inside the app — open **Settings** with
`F10` (Theme section), preview with `↑/↓`, `Enter` saves.

```toml
[theme]
name        = "dracula"   # bundled preset; omit for the default

# ── Surface ─────────────────────────────────────────────────────
accent      = "#cba6f7"   # active borders, cursor, highlights — overrides the preset
inactive    = "#a6adc8"   # inactive panel borders & titles
selected_bg = "#313244"   # selected list-row background
muted       = "#45475a"   # decorative separators / barely-visible borders

# ── Text ─────────────────────────────────────────────────────────
foreground  = "#cdd6f4"   # main body text
                          # default Color::Reset → inherit terminal fg
dim         = "#6c7086"   # secondary text, hints, counters
placeholder = "#6c7086"   # empty-input "type here…" hints

# ── Feedback ─────────────────────────────────────────────────────
success     = "#a6e3a1"   # ✓ and success messages
error       = "#f38ba8"   # ✕ and error messages

# ── Item-type colors ────────────────────────────────────────────
item_login    = "#89b4fa"
item_card     = "#cba6f7"
item_identity = "#f9e2af"
item_note     = "#a6e3a1"
item_ssh      = "#b4befe"
item_favorite = "#f9e2af"

# ── Decorative starfield (splash + login background) ────────────
star_dim    = "#262248"
star_mid    = "#5a5494"
star_bright = "#b9b2f8"
```

Hex values may be quoted (`"#cba6f7"`) or unquoted (`#cba6f7`). Wrong-length or malformed values silently fall back to the default for that key — they do not abort startup.

`foreground = Color::Reset` (the default) is intentional: bytewarden inherits whatever foreground colour your terminal uses, so light-background terminals stay readable. Set it to a hex value if you want to lock a specific tone regardless of terminal.

---

## Login screen

Three text fields plus a one-time-code field that only appears when the backend triggers a new-device challenge, three checkboxes, and a feedback strip:

```
┌─ Login ─────────────────────────────────────────────────────────┐
│  Server: https://vault.bitwarden.com                            │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ https://vault.bitwarden.com█                             │   │
│  └──────────────────────────────────────────────────────────┘   │
│  Email:                                                          │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ you@example.com                                           │   │
│  └──────────────────────────────────────────────────────────┘   │
│  Master Password:    (F2: reveal)                                │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │ ●●●●●●●●                                                  │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ☑ Save email                                                    │
│  ☐ Auto-lock after 15 min                                        │
│  ☑ Keep session                                                  │
│  ───────────────────────────────────────────────────────────     │
│  ⠋ Logging in…                                                   │
└──────────────────────────────────────────────────────────────────┘
```

### Keys

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Cycle: Server → Email → Password → [OTP] → Save email → Auto-lock → Keep session |
| `← → Home End` | Move cursor in text fields |
| `Backspace` / `Delete` | Delete character |
| `Space` | Toggle the focused checkbox |
| `F2` | Reveal / hide master password |
| `Enter` | Login / unlock — or apply the Server URL when on the Server field |
| `Alt+K` | **API-key login** (reads `BW_CLIENTID` and `BW_CLIENTSECRET` from env) |
| `Alt+S` | **SSO login** — opens the browser and blocks until the federated callback arrives |
| `F1` | Help popup, scoped to the login screen |
| `Ctrl+C` | Quit |

### Server field

The Server field is pre-filled from `bw status` so you always see which backend you're hitting. Editing the value and pressing `Enter` (or leaving the field with `Tab`) calls `bw config server <url>` for you. Self-hosted Bitwarden_RS / Vaultwarden instances work out of the box — no extra wrapper script needed.

### Alternative login methods

- **API key** (`Alt+K`) — headless flow that reads `BW_CLIENTID` and `BW_CLIENTSECRET` from the environment. After it succeeds the vault is *Locked* (not unlocked) — bytewarden tells you to type your master password and hit `Enter` to finish.
- **SSO** (`Alt+S`) — opens the browser via `bw login --sso`, blocks until the federated callback returns, and again leaves the vault *Locked* awaiting your master password.

### OTP / "new device" verification

If Bitwarden detects a new device, the login fails with `device verification required` and bytewarden injects an extra **Verification Code** field above the checkboxes plus a hint strip:

```
✉ Check your email for the verification code.
```

Paste the code, press `Enter`, and you're in.

### Two-factor login

If your account has a permanent second factor enrolled, bytewarden detects the `two-step login` prompt from `bw` and surfaces a **Two-step Code** field plus a method chip below it. Three methods are supported:

| Method | bw flag | Hint label |
|---|---|---|
| Authenticator (TOTP) | `--method 0` | TOTP from your authenticator app |
| Email | `--method 1` | sent to your email |
| YubiKey | `--method 3` | touch your YubiKey |

Default is **Authenticator** — the most common case. Press `← →` while the cursor is on the Two-step Code input to cycle through the methods. Type the code (or touch your YubiKey, depending on the method) and press `Enter`.

Under the hood bytewarden invokes `bw login --method <N>` and feeds the code via stdin so it never reaches `argv` (and therefore never `ps`). Methods bw doesn't expose via CLI (Duo, WebAuthn, U2F) are not supported because they require a browser callback that doesn't fit the blocking-CLI flow.

### Feedback strip states

| State | Display |
|-------|---------|
| Checking session | `⠋ Checking session…` (spinner, accent) |
| Logging in | `⠋ Logging in…` (spinner, accent) |
| Loading vault | `⠋ Loading vault…` (spinner, accent) |
| Device-verification OTP needed | `✉ Check your email for the verification code.` (accent) |
| Two-factor needed | `Two-step Code` row + method chip (`← →` cycles Authenticator / Email / YubiKey) |
| Success | `✓ Loaded ✓` (success) |
| Wrong credentials | `✕ Invalid credentials. Please try again.` (error) |
| Wrong device-verification OTP | `✕ Invalid verification code. Please try again.` (error) |
| Wrong 2FA code | `✕ Invalid two-factor code. Please try again.` (error) |

---

## Session resume

On every startup bytewarden runs `bw status` *before* showing the login screen and fast-paths the UI:

| `bw status` | What bytewarden does |
|-------------|----------------------|
| `unauthenticated` | Login form — enter email and master password. |
| `locked` | Email pre-filled from the CLI's known account; cursor jumps to the password field. |
| `unlocked` | Active session found; vault loads immediately, login screen skipped. |

A `⠋ Checking session…` spinner is visible while the check runs.

The `unlocked` fast-path triggers when **either** of these is true:

1. `$BW_SESSION` is exported and the bw CLI accepts it.
2. The optional **Keep session** file (see next section) carries a session key that the CLI accepts.

If the saved key is rejected (server changed, logged out elsewhere, …), bytewarden falls back to the *Locked* path and surfaces:

```
✕ Saved session is no longer valid. Please log in again.
```

---

## Keep session (per-terminal persistence)

Toggling **Keep session** on the login screen makes the unlocked `bw` session key persist across launches as long as your parent shell is still alive — no need to `eval $(bw unlock --raw)` yourself.

### How it works

- After a successful login bytewarden writes the session key to:
  - `${XDG_RUNTIME_DIR}/bytewarden/session-{PPID}` if `XDG_RUNTIME_DIR` is set (the standard Linux per-user runtime dir, wiped at logout).
  - `/tmp/bytewarden-$USER/session-{PPID}` otherwise.
  - File mode `0600`, directory mode `0700`.
- On every launch, **before** building the bw adapter, bytewarden:
  1. Removes any orphan files whose PPID no longer points at a live process.
  2. If the file for the **current** PPID exists and the parent is still alive, hydrates `$BW_SESSION` from it so the rest of the boot path picks it up.
- Locking the vault, logging out, or unticking the checkbox erases the file immediately.

### Caveats

- Only Unix (`std::os::unix::process::parent_id`, `kill -0`, POSIX perms). bytewarden is already Linux/macOS-only because of the clipboard backends.
- Under `tmux` / `screen` the "parent" is the multiplexer client, not the shell inside the pane — so the session lives until the multiplexer dies.
- `exec`-replacing your shell changes the PPID and invalidates the file (you'll be asked for the master password again).
- Session files older than **24 hours** are treated as stale and dropped at startup or load — independent of whether their PPID still resolves to a live process. This caps the window in which a recycled PID could keep an orphan key readable.

---

## Vault screen

### Layout

```
┌─[0]-Status──────────┐  ┌─[/]-Search──────────────────────────────┐
│                     │  │ ⌕ type to filter…                        │
│                     │  └──────────────────────────────────────────┘
│                     │  ┌─[3]-Vault──────────── 12 of 87 ──────────┐
└─────────────────────┘  │ ★    [Login]    GitHub                   │
┌─[1]-Folders────1/4──┐  │   🔒 [Login]    AWS Production           │
│ ▶ 📁 All folders 87 │  │      [Login]    AWS Sandbox              │
│   (No folder)    1  │  │   🔒 👥 [Login] Acme / SSO               │
│   ─────────────     │  │      👥 [Card]  Acme / Corp Visa         │
│   Work          54  │  │ ★    [Identity] Personal                 │
│   Personal      32  │  │      [Note]     Phrase                   │
└─────────────────────┘  │                                          │
┌─[2]-Items────1 of 8─┐  │                                          │
│ ▶  All Items     87 │  └──────────────────────────────────────────┘
│ ★ Favorites      12 │  ┌─[4]-Command Log─────────────────────────┐
│ 󰌋 Login          54 │  │ $ bw status                              │
│ 󰻷 Card            8 │  │ ✓ Unlocked                               │
│ 󰀉 Identity        3 │  │ $ bw list items --session ***            │
│ 󰎞 Secure Note     7 │  │ ✓ 87 items loaded                        │
│ 󰣀 SSH Key         3 │  └──────────────────────────────────────────┘
│ ─────────────       │
│ 󰩺 Trash           4 │
└─────────────────────┘
                      Tab: panel  |  /: search  |  ?: help  |  F1: help
```

### Panels

| ID | Label | Contents |
|----|-------|----------|
| `[0]` | Status | Action feedback — spinner, ✓, ✕. Read-only. |
| `[1]` | Folders | Folder filter sidebar (All / No folder / one row per folder). |
| `[2]` | Items | Item-type filter sidebar (All / Favorites / Login / Card / …). |
| `[/]` | Search | Live fuzzy-search input. |
| `[3]` | Vault | Main item list. |
| `[4]` | Command Log | Last 50 `bw` calls with their result — session keys redacted. |

### Panel navigation

| Key | Action |
|-----|--------|
| `0` | Focus **[0]-Status** |
| `1` | Focus **[1]-Folders** |
| `2` | Focus **[2]-Items** |
| `3` | Focus **[3]-Vault** |
| `4` | Focus **[4]-Command Log** |
| `/` | Focus **[/]-Search** |
| `Tab` | Cycle: Search → Folders → Items → List → CmdLog → Search |

Number keys `0`–`4` are disabled while Search is focused so you can type them as part of a query.

### List actions (Search & List panels)

| Key | Action |
|-----|--------|
| `j` / `↓`, `k` / `↑` | Move selection |
| `PgUp` / `PgDn` | Page (10 rows) |
| `Enter` / `l` | Open item detail |
| `Alt+N` | **New item** — opens the create flow |
| `Alt+U` | Copy username to clipboard |
| `Alt+C` | Copy password to clipboard |
| `Alt+F` | Toggle favorite ★ |
| `Alt+S` | **Sync** vault with the server |
| `Alt+D` | **Delete item** — opens confirmation popup |
| `Alt+R` | Restore (only meaningful inside the Trash filter) |
| `Alt+L` / `Alt+Q` | **Lock** vault |
| `Alt+O` | **Log out** — removes the account from the local CLI |
| `Alt+I` | Show the user's **fingerprint phrase** (toast) |
| `Alt+G` | Open the **password generator** (popup) |
| `Alt+E` | Open the **export** popup |
| `Alt+M` | Open the **import** popup |
| `Alt+W` | Create a text **Send** (popup) |
| `Alt+B` | View **memberships** (organisations + collections) |
| `F1` | Help popup — context-aware |
| `F10` | Open **Settings** (Theme preset picker · Security · Advanced) |
| `Ctrl+C` | Quit |

All `Alt+` shortcuts also work while Search is focused.

### Item indicators

Each list row carries up to three icon prefixes before the `[Type]` column:

| Icon | Meaning |
|------|---------|
| `★` | Favorite — toggled with `Alt+F` |
| `🔒` | Reprompt-protected — secret-exposing actions ask for the master password (see [Reprompt](#reprompt-master-password-reverify)) |
| `👥` | Belongs to an organisation (`organizationId` is set) — shared across collections |

Indicators are independent — an item can be all three at once, or none.

### Search

Press `/` to focus the search bar. Fuzzy search runs across **name**, **username** and **URI** — results update live and re-rank as you type.

#### URL-only filter

Prefix the query with `url:` to narrow the match to login URIs only — the equivalent of `bw list items --url <url>` in the CLI. Useful for "what credentials do I have for this site?" lookups when the item name is unrelated to the domain.

| Query | Behaviour |
|---|---|
| `url:github` | Only items whose URIs contain `github`, in their original list order (no fuzzy ranking) |
| `url:https://example.com/login` | Only items whose URIs contain that exact substring |
| `url:` (bare prefix) | No narrowing — same as an empty query |

Non-prefixed queries keep the regular fuzzy ranking over name + username + URI + notes. While focused:

- Plain characters extend the query.
- `Backspace` pops the last character.
- `Esc` clears the query and returns to the list.
- `j`/`k` and `Enter` navigate / open the highlighted item.
- `Tab` cycles to the next panel.

The score (descending) is roughly:

| Hit | Score |
|-----|-------|
| Name substring | 100 (+20 if it's a **prefix** match) |
| Name subsequence (chars in order, not contiguous) | 50 |
| Username substring / subsequence | +30 / +10 |
| Any URI substring | +10 (one-shot) |
| Notes substring | +5 |

Items scoring 0 are dropped.

### Items filter `[2]`

The default view (All Items) shows everything. Use the sidebar to narrow:

```
All Items · ★ Favorites · 󰌋 Login · 󰻷 Card · 󰀉 Identity · 󰎞 Secure Note · 󰣀 SSH Key · 󰩺 Trash
```

Selecting **Trash** triggers a separate `bw list items --trash` fetch — trashed items are not part of the main in-memory list. Per-row counts on the right show how many items match each filter for the current folder.

### Folders panel `[1]`

| Key | Action |
|-----|--------|
| `j` / `k`, `↑` / `↓`, `PgUp`/`PgDn` | Move selection |
| `Enter` | Apply folder/collection filter |
| `Alt+N` | **New folder** (popup) |
| `Alt+R` | **Rename** focused folder (popup) |
| `Alt+D` | **Delete** focused folder (confirm) |
| `Tab` / `Esc` | Cycle focus away |

The folder/collection filter is ANDed with the item-type filter. The two fixed rows at the top are:

- `📁 All folders` — no folder/collection constraint.
- `(No folder)` — items with `folder_id == null`.

Below the separator, the panel shows two sections in one scrollable list:

- **Personal folders** (icon `📁`) — your private organisational containers. `Alt+N` / `Alt+R` / `Alt+D` work here.
- **Collections** (icon `👥`, labelled `"Org / Name"`) — shared containers from every organisation you're a member of. Read-only for now (assignment via `bw move` is a follow-up); the rows are filterable like folders, and the per-row count tells you how many of your visible items belong to that collection. Personal-only accounts don't see this section at all.

Deleting a folder leaves its items intact; their `folder_id` is cleared by `bw`. Collections cannot be deleted from bytewarden.

---

## Item detail

Two modes: **read** (default) and **edit** (`Alt+E`). Both walk the same field list — they just render and react differently.

### Read mode

| Key | Action |
|-----|--------|
| `j` / `k`, `↑` / `↓`, `Tab` / `Shift+Tab` | Move between fields |
| `PgUp` / `PgDn` | Same as `k` / `j` |
| `F2` | Reveal / hide selected hidden field |
| `Alt+C` | Copy selected field to clipboard |
| `Alt+E` | **Enter edit mode** |
| `Alt+M` | **Move into your organisation** — opens the assign-collections popup pre-filled with the user's org. Only when the item is personal and the user has exactly 1 organisation; multi-org accounts get an error toast asking them to use `bw move` from shell. |
| `Alt+D` | **Delete item** — confirm popup |
| `Alt+X` | **Check password against HaveIBeenPwned breaches** (toast) |
| `Alt+A` | **Upload attachment** (popup) |
| `Alt+S` | **Download** focused attachment (popup with destination path) |
| `Alt+Del` | **Delete** focused attachment (confirm) |
| `Alt+R` | **Restore** item (only inside Trash) |
| `Esc` / `h` | Back to vault |

Hidden fields (Password, Card Number, CVV, TOTP, SSN, Passport, License, custom hidden fields) render as `●●●●●●●●` until `F2` is pressed. Navigating away re-hides the field.

The **HIBP check** (`Alt+X`) hashes the password locally and queries [HaveIBeenPwned](https://haveibeenpwned.com) k-anonymity range API. The result is shown as a toast: "0 breaches" (safe so far) or "found in N breaches".

### Edit mode

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous field (wraps) |
| `↑` / `↓` | Next / previous field (clamps) |
| `← → Home End` | Move cursor within field |
| `Backspace` / `Delete` | Delete character |
| `F2` | Reveal / hide hidden field while editing |
| `Enter` | **Save** — calls `bw edit item` |
| `Esc` | Cancel — back to read mode (no changes saved) |
| `Alt+G` | Generate password into the focused hidden field |
| `Alt+N` | Add custom field |
| `Alt+R` | Rename focused custom field (popup) |
| `Alt+T` | Cycle custom-field type (text → hidden → boolean → linked) |
| `Alt+U` | Add a URL row (login items only) |
| `Alt+L` | Assign collections (popup) — only on the read-only **Collections** row of an organisation item |
| `Alt+Del` | Remove focused custom field or URL row |

#### Collections assignment

Items that belong to a Bitwarden **organisation** show an extra read-only **Collections** row at the bottom of the edit form, listing the collection names the item is currently shared into. Move the cursor to that row and press `Alt+L` to open a multi-select popup:

```
┌─ Assign collections ───────────────────────────────────┐
│  2 of 4 selected                                       │
│  ▸ [x]  Engineering                                    │
│    [x]  Ops                                            │
│    [ ]  Marketing                                      │
│    [ ]  Sales                                          │
│  j/k or ↑↓ to navigate · Space to toggle · Enter ·Esc  │
└────────────────────────────────────────────────────────┘
```

Bitwarden requires every organisation-owned item to live in **at least one** collection — empty selection is rejected with an inline error strip so you can fix it before the save round-trip.

Personal-vault items don't show this row. Creating an item directly into an organisation and moving an existing item between vaults (`bw move`) are not supported yet — for now bytewarden only edits the collection set of items that *already* belong to an org.

The **Type** field is read-only. **Fingerprint** on SSH key items is read-only too (recomputed by `bw` on save). All other fields are editable. Save is atomic via `bw edit item` — the local item list is updated immediately on success.

### Custom fields

Logins / cards / notes / identities / SSH-key items can carry an arbitrary number of user-defined fields. Type discriminants follow the bw enum:

| Type | Render |
|------|--------|
| `0` Text | Plain editable line |
| `1` Hidden | Masked unless revealed with F2 |
| `2` Boolean | Same surface as text — value is `"true"` or `"false"` |
| `3` Linked | **Read-only** in the TUI — picking a target field needs UI bytewarden does not have yet. Linked fields created in the official Bitwarden GUI survive every edit/save round-trip with their `linkedId` intact; the `Alt+T` cycler refuses to touch them and shows an explanatory toast. |

`Alt+T` cycles the focused custom field through the four types and refreshes the masking flag immediately. `Alt+R` opens an inline rename popup.

### Multi-URI logins

Login items support any number of URIs. In edit mode each URI takes two rows:

```
URL 1
[https://github.com                                             ]
URL 1 Match
[Domain                                                          ]
URL 2
[https://github.io                                              ]
URL 2 Match
[                                                                ]
```

`URL Match` accepts the bw labels (`Domain`, `Host`, `Starts With`, `Exact`, `Regex`, `Never`, case-insensitive) or the bare digits `0`–`5`. Empty means "use the account-wide default" (Domain).

`Alt+U` adds a new URL row at the end; `Alt+Del` on a URL or its match row removes the slot.

---

## Create item

Press `Alt+N` from the vault list (or while the Search bar is focused) to start a new item.

### Step 1 — pick a type

| Key | Action |
|-----|--------|
| `j` / `k`, `↑` / `↓` | Move selection |
| `Tab` / `Shift+Tab` | Same, wraps |
| `Enter` | Confirm and go to fields |
| `Esc` | Cancel — back to vault |

Supported types: **Login, Secure Note, Card, Identity, SSH Key**.

### Step 2 — fill the form

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Next / previous field (wraps) |
| `↑` / `↓` | Next / previous field (clamps) |
| `← → Home End` | Move cursor (or cycle the **Organization** row) |
| `Backspace` / `Delete` | Delete character |
| `F2` | Reveal / hide hidden field |
| `Alt+G` | Generate password into the focused hidden field |
| `Alt+L` | Pick collections — only on the read-only **Collections** row |
| `Enter` | **Create** — calls `bw create item` |
| `Esc` | Cancel |

The **Name** field is required. On success the new item is inserted into the local list and the vault screen is shown with it pre-selected.

### Creating directly in an organisation

If your account is a member of one or more Bitwarden organisations, the create form gets an extra **Organization** row at the bottom. Default is `Personal` (item lands in your private vault). Move the cursor onto that row and press `← →` to cycle through `Personal · Acme · Beta · …`.

Picking a real organisation injects a sibling **Collections** row right below it. Press `Alt+L` to open the multi-select popup and pick which collections the new item should belong to. Bitwarden requires every org item to live in at least one collection — `Enter` to create rejects an empty selection inline before paying for the network round-trip.

Personal-only accounts don't see either row, and the create flow stays identical to the previous behaviour.

---

## Folders

Bytewarden ships full folder CRUD inline — no shelling out to `bw create folder` yourself.

| Where | Key | Action |
|-------|-----|--------|
| Folders panel `[1]` | `Alt+N` | **New folder** (popup; type a name → Enter) |
| Folders panel `[1]` | `Alt+R` | **Rename** focused folder |
| Folders panel `[1]` | `Alt+D` | **Delete** focused folder (confirm) |

Folder operations refresh the sidebar silently — no toast spam — and update the per-folder item counts.

---

## Password generator

`Alt+G` from the vault opens a stand-alone generator. From a hidden field in edit / create mode it opens with a *return target*: pressing `Alt+U` writes the result back into that field.

### Modes

- **Password** — random characters from the enabled classes.
- **Passphrase** — diceware-style word list with separator and capitalisation toggles.

### Keys

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab`, `↑` / `↓` | Move between options |
| `Space` / `← →` | Toggle the focused option / adjust value |
| `Enter` | **Generate** (manual trigger — config changes do not auto-regenerate) |
| `Alt+C` | Copy result to clipboard |
| `Alt+U` | Use result in the originating form (when launched there) |
| `Esc` | Close |

### Options

Password mode:

| Option | Default | Notes |
|--------|---------|-------|
| Length | 16 | Clamped to **5** by the adapter |
| Uppercase | on | |
| Lowercase | on | |
| Numbers | on | |
| Special | off | |
| Avoid ambiguous (`O0`, `Il1`…) | off | |

Passphrase mode:

| Option | Default | Notes |
|--------|---------|-------|
| Words | 4 | Clamped to **3** by the adapter |
| Separator | `-` | Any single character |
| Capitalize | off | |
| Include number | off | |

The adapter validates up front: in password mode at least **one** character class must be enabled, otherwise the generator surfaces an error rather than returning garbage.

---

## Export / Import

### Export (`Alt+E` from the vault)

Popup with a format picker and an output path:

| Format | Notes |
|--------|-------|
| `JSON` | Plaintext — choose a safe location. |
| `CSV` | Plaintext — choose a safe location. |
| `Encrypted JSON` | Encrypted with your account key. **Only re-importable into this same Bitwarden account.** |

Defaults: format = JSON, path = `~/Downloads/bytewarden-export-<timestamp>.<ext>`. The path auto-refreshes when you cycle the format unless you have already edited it.

| Key | Action |
|-----|--------|
| `Tab` / `Shift+Tab` | Switch field |
| `Space` / `← →` | Cycle format |
| Text input | Edit path |
| `Enter` | Export |
| `Esc` | Cancel |

### Import (`Alt+M` from the vault)

Popup with a free-form format string and a path. Run `bw import --formats` to see the full list — common ones include `bitwardenjson`, `lastpasscsv`, `1password1pux`, `chromecsv`, `keepass2xml`, etc. Default format is `bitwardenjson`.

> Imported items are appended to your vault — duplicates are not deduped.

---

## Send (text)

`Alt+W` from the vault opens the Send-create popup. Bitwarden Send creates a self-destructing link to the text content.

| Field | Notes |
|-------|-------|
| Name | User-visible label |
| Expires in | 1–31 days, adjusted with `← →` |
| Content | Single line — for multi-line use `bw send` directly |

| Key | Action |
|-----|--------|
| `Tab` / `↑↓` | Switch field |
| `← →` | Adjust days (when on the Days field) |
| `Enter` | Create — the URL is auto-copied to your clipboard on success |
| `Esc` | Cancel |

---

## Memberships (organisations + collections)

`Alt+B` from the vault opens a read-only view of your Bitwarden organisations and the collections you can see inside each. Personal-only accounts see a friendly empty state.

| Key | Action |
|-----|--------|
| `Esc` / `Enter` | Close |

---

## Attachments

From the detail screen:

| Key | Action |
|-----|--------|
| `Alt+A` | **Upload** a new attachment to the focused item — popup asks for the source file path, then runs `bw create attachment`. |
| `Alt+S` (on an Attachment row) | **Download** the focused attachment — popup pre-fills `~/Downloads/<filename>`, suffixing with `_1`, `_2`, … if a file already exists at that path. Edit the path before pressing Enter to save anywhere else. Runs `bw get attachment`. |
| `Alt+Del` (on an Attachment row) | **Delete** the focused attachment — confirmation popup, then `bw delete attachment`. The item is reloaded from the server so the row count drops immediately. |

Attachments are listed at the bottom of the detail view as `<file_name>   (<size>)`. Move the cursor onto an Attachment row before pressing `Alt+S` / `Alt+Del`; pressing them on any other row shows a "Move to an attachment row first." toast.

---

## Mouse support

Mouse capture is enabled at boot and disabled cleanly on exit.

| Action | Effect |
|--------|--------|
| Click panel | Focus that panel |
| Click list item | Select it |
| Click same item again | Open detail (double-click semantics) |
| Click filter row | Apply filter immediately |
| Scroll wheel | Scroll the hovered panel (list / cmdlog / filters / help) |
| `Shift` + scroll wheel | Horizontal pan in the help popup |
| Click detail field | Select field |
| Click same detail field again | Toggle reveal on hidden fields |
| Click detail header (back arrow row) | Return to vault |
| Click login form field | Focus that field (or toggle the checkbox) |

---

## Help popup (F1)

`F1` from any of the four main screens (Login, Vault, Detail, Create) opens a context-aware help popup. The content is **scoped**:

- On Login → only login keys + alternative login methods.
- On Vault → "Vault — global" and "Vault — actions (Alt)" plus a sub-section that depends on the focused panel (Folders, Items, Search, List, Command log).
- On Detail → Read mode + Edit mode (the title shows which one you're in).
- On Create → Type picker (when choosing) or Fill fields (when filling).

The popup is **scrollable** — handy on small terminals or when you want to scan a long section.

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll down 1 line |
| `k` / `↑` | Scroll up 1 line |
| `h` / `←` | Pan left 2 columns |
| `l` / `→` | Pan right 2 columns |
| `PgUp` / `PgDn` | Scroll 8 lines |
| `Shift+H` / `Shift+L` | Pan 16 columns |
| `Home` | Top-left |
| `End` | Bottom |
| `q` / `F1` / `Esc` | Close |

Mouse: wheel scrolls vertically; `Shift+wheel` pans horizontally. The popup border draws `▲` / `▼` / `◀` / `▶` indicators where there is hidden content.

Popups (Generator, Export, Import, …) deliberately do not open Help on top of themselves — they each carry their own self-contained instructions in the footer.

---

## Command log

Panel `[4]`. Every `bw` invocation is logged with:

- The redacted command (session keys are replaced with `***`).
- Whether it succeeded (`✓`) or failed (`✕`).
- A short detail line — for reads, often a count; for errors, the underlying CLI stderr.

Capacity: last **50** entries (FIFO).

| Key | Action |
|-----|--------|
| `j` / `↓` | Scroll up 1 line |
| `k` / `↑` | Scroll down 1 line |
| `PgUp` / `PgDn` | Scroll 5 lines |

Passwords, TOTP codes and clipboard contents are never logged in the clear.

### Persistent debug log

Set `BYTEWARDEN_DEBUG=1` in the environment before launching to also append every command-log line to `~/.bytewarden.log` (mode `0o600`). Useful when you want to scroll back through a session that exceeded the 50-entry in-memory window or share a reproduction with a maintainer. The format is one line per event with a UTC timestamp:

```text
2026-05-03T14:23:11Z  ✓  bw status                                 → Unlocked
2026-05-03T14:25:40Z  ✕  bw sync --session ***                     → bw sync timed out after 30s
```

Same redaction rules as the in-app panel — session keys are already masked before reaching the file. Unset the variable (or leave it unset) to disable; the check is a cheap `is_err()` and the file is never opened.

---

## Reprompt (master-password reverify)

Items marked with the Bitwarden **reprompt** flag (right-click → "Master password re-prompt" in the official GUI) ask the user to reconfirm their master password before exposing any secret. Bytewarden honors this flag on these actions:

| Trigger | Where |
|---|---|
| `Alt+C` (copy password) | Vault list |
| `Alt+C` (copy focused row) | Detail screen, only when the row is hidden — password / TOTP / hidden custom field |
| `F2` (reveal hidden) | Detail screen (read-mode), only on the false→true transition |
| `F2` (reveal hidden field) | Detail screen (edit-mode), only when the focused row is currently masked |

Non-secret actions on the same item — copying the username or URL, viewing the detail page, entering edit mode — are **not** gated; they expose nothing the user can't already see by virtue of the vault being unlocked.

When triggered, a centered popup appears:

```
┌─ Master password required ─────────────────────────────┐
│  This item asks to re-verify before copying the pass…  │
│  Master password                                       │
│  ┌─────────────────────────────────────────────────┐   │
│  │ ●●●●●●●●█                                        │   │
│  └─────────────────────────────────────────────────┘   │
│  Enter to verify · Esc to cancel                       │
└────────────────────────────────────────────────────────┘
```

Type the master password, hit `Enter`. Bytewarden runs `bw unlock` against the input — on success the deferred action fires and the in-memory session key is silently rotated to the new one bw issued; on failure the popup stays open with an error strip and the buffer is cleared so you can retry.

**No caching.** Every protected action re-prompts. This matches the official Bitwarden GUI behaviour and is intentional: caching would defeat the protection on rapid-fire copy-then-reveal sequences.

## Auto-lock

Tick **Auto-lock** on the login screen (or set `auto_lock = true` in the config file) and bytewarden locks the vault after `lock_after_minutes` minutes of inactivity. Activity is any keypress; the timer resets on each one.

The check only fires from the Vault and Detail screens — typing into a popup or sitting on the login screen doesn't expose anything anyway.

---

## Architecture

Hexagonal (ports & adapters):

```
 main ──► tui ──► flows ──► ports ◄── adapters
                     ▲
                     └── domain (used by every layer above)
```

- `domain/` — pure types and rules (Item, LoginData, CardData, IdentityData, SshKeyData, Field, UriData, ItemFilter, fuzzy_score, input validators, …). No I/O.
- `ports/` — trait abstractions: `VaultPort`, `ClipboardPort`, `SettingsPort`, `PasswordGeneratorPort`.
- `adapters/` — concrete implementations:
  - `BwCliAdapter` — spawns the `bw` binary; passwords go through `BW_PASS_INPUT` env var (never argv); base64 encoding done in-process; OTP detection by regex on stderr/stdout.
  - `BwGeneratorAdapter` — `bw generate <flags>`, stateless.
  - `SystemClipboardAdapter` — picks `wl-copy` / `xclip` / `xsel` / `pbcopy` at runtime.
  - `TomlSettingsAdapter` — hand-rolled TOML reader/writer that preserves unknown sections (the `[theme]` block in particular).
- `tui/` — the driving adapter: `App` state container, `Screen` enum, input router (per-screen handlers), view router (per-screen renderers), action queue (`PendingAction`), session-file helper.
- `main.rs` — composition root. Hydrates `BW_SESSION` from the keep-session file (if any) and wires concrete adapters into `tui::run`.

The model is synchronous (no async runtime). Blocking calls to `bw` are smoothed with a one-frame delay so the spinner is always drawn *before* the call: tick 1 sets `Running("…")` + queues a `PendingAction`, tick 2 executes the call, tick 3 renders `Done(…)` / `Error(…)` which auto-expires after ~1.5 s.

Every `bw` call that touches the network has a wall-clock timeout. The OS TCP timeout (~30 s on Linux, longer on macOS) would otherwise let a flaky connection freeze the TUI for the full duration. Concrete budgets:

| Operation | Budget |
|---|---|
| `bw status` | 4 s |
| Login (master / API-key / OTP) | 30 s |
| `bw config server`, `bw logout` | 10 s |
| `bw sync` | 30 s |
| Item / folder CRUD, HIBP check, send | 15 s |
| Export / import / attachment up- & download | 60 s |
| SSO login | **no timeout** — depends on the user finishing the browser flow |

Local-only calls (unlock, list cached items, get TOTP from the local store, list folders / orgs / collections, get fingerprint) deliberately keep no timeout — they finish in milliseconds and a stuck `bw` there indicates a bigger problem than a slow network.

A timeout firing surfaces as a normal failure toast (`✕ Sync failed: bw timed out after 30s`) and the action queue returns to `Idle` — the user can retry.

Free-form inputs are pre-validated before the corresponding `bw` call:

| Input | Check | Error toast |
|---|---|---|
| Email on the login form | Must contain `@` and a dotted domain | `Email is missing '@'.` etc. |
| Server URL | Must start with `http://` or `https://` and have a host | `Server URL must start with http:// or https://.` |
| Export path | Parent directory exists, file does not already exist (no silent overwrite of plaintext) | `File already exists: …` |
| Import path | Refers to an existing file | `Import file not found: …` |
| Folder name | Unique among existing folders (case-insensitive) | `"Work" is already used.` |
| Custom field label | Unique among sibling custom fields on the same item | `"API_KEY" is already used.` |

Validators live in `domain::validation` as pure functions, so they are unit-tested without plumbing fakes.

When the terminal is below the minimum that the layouts assume (60 columns × 18 rows) the renderer steps out of the way and shows a single centred "Terminal too small — resize to at least 60×18" message. Resize, the next frame redetects the new size, and the regular UI comes back.

In-memory secrets are wrapped end-to-end:

- **Session key, master-password buffer, OTP / 2FA buffer** are stored in [`zeroize::Zeroizing<String>`](https://docs.rs/zeroize). When the wrapper drops (lock, logout, OTP submitted, password cleared on success) every byte of the underlying string is overwritten with zeroes via a `write_volatile` the compiler can't optimise away.
- **Every vault-data payload** (`Item`, `LoginData`, `CardData`, `SshKeyData`, `IdentityData`, `Field`, `UriData`, `Attachment`) derives `Zeroize` + `ZeroizeOnDrop`, so the entire `app.items` / `app.trashed_items` cache is scrubbed on lock / logout / app shutdown. Clones the flows pass around (favourite toggle, edit-mode entry, copy-staging…) and the parallel lowercased search cache get the same treatment.
- **JSON intermediates** carrying credentials (`get_item_json` result, the patched payload built before `bw edit item`) are wrapped in `Zeroizing<String>` at the boundary so the heap allocation is freed with zeros.
- **Edit-form buffers** (`EditField.value`) and the **generator result** (`GeneratorState.result`) live in `Zeroizing<String>` too — a freshly-generated password, or a password the user is mid-edit, never sits unscrubbed in the heap waiting for the allocator to reclaim it.

This narrows the window in which a heap dump or a swap-out could leak credentials after the user has already locked.

The composition root in `main.rs` carries no `unsafe` blocks. Hydration of the keep-session key happens via `BwCliAdapter::new_with(seed)` — the seed is passed straight to the adapter constructor instead of being injected into the process environment, so we never touch `std::env::set_var` (which is `unsafe` in edition 2024). The adapter still falls back to `$BW_SESSION` from the inherited environment when the seed is `None`, so users who export the variable manually keep working.

---

## Development

```bash
cargo build           # debug build
cargo build --release # optimised build (lto, opt-level=s, strip)
cargo run             # run from source
cargo test            # run all tests
cargo clippy -- -D warnings   # zero-warning lint
cargo fmt             # format
```

A GitHub Actions workflow at `.github/workflows/ci.yml` runs `cargo fmt --check`, `cargo clippy -D warnings`, `cargo build`, and `cargo test` on every push and PR to `main`. A separate informational coverage job runs `cargo llvm-cov --summary-only` and is allowed to fail (the testable layer is at ~95% but the project total is around 30% — see "Testing & coverage").

Useful extra:

```bash
cargo run -- --help   # nope — bytewarden has no flags, every option is a key
```

### Project layout

```
src/
├── main.rs
├── lib.rs
├── domain/                        — pure entities & rules
├── ports/                         — trait abstractions
├── adapters/
│   ├── bw_cli/{mod,codec,json,process}.rs
│   ├── bw_generator.rs
│   ├── clipboard_system.rs
│   └── settings_toml.rs
└── tui/
    ├── app.rs                     — global state container
    ├── action.rs                  — PendingAction queue + ActionState
    ├── screens.rs                 — Screen / Focus / LoginField enums
    ├── theme.rs                   — Theme struct + parser
    ├── session_file.rs            — Keep-session file helper
    ├── edit_field.rs              — Edit/create form field model
    ├── detail_fields.rs           — Detail-screen row model
    ├── folders.rs                 — FolderFilter + sidebar helpers
    ├── flows/                     — per-feature action handlers
    ├── input/                     — per-screen key handlers + mouse + nav
    └── view/                      — per-screen renderers + shared widgets
```

---

## Testing & coverage

The crate has **252 unit tests + 4 doctests**, all run with `cargo test`.

```bash
cargo test                      # everything
cargo test --quiet              # one line per test
cargo test domain               # only the domain layer
cargo test --lib edit_field     # one module
```

Coverage is measured with [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov):

```bash
cargo install cargo-llvm-cov
cargo llvm-cov --summary-only   # tabular summary by file
cargo llvm-cov --html           # navigable HTML report → target/llvm-cov/html
cargo llvm-cov --open           # generate + open in the browser
```

### Coverage status

| Layer | Lines |
|-------|-------|
| `domain/` (pure) | ~98% |
| `adapters/` (bw_cli/codec, bw_cli/json, bw_generator::build_args, settings_toml) | ~94% |
| `tui/` testable helpers (folders, edit_field, detail_fields, theme, session_file, flows::item_json) | ~95% |
| **TOTAL (all 12 k LOC)** | **~29%** |

The "lo que apuntamos a cubrir" capa is at **~95%**. The total is brought down by:

- `tui/view/*` (~3 200 lines) — render code, untested without snapshot tests.
- `tui/input/*` (~1 400 lines) — handlers, untested without a synthetic event harness.
- `tui/flows/*` except `item_json` — untested without fakes for the four ports.
- `adapters/bw_cli/{mod,process}.rs` — untested without a fake `bw` binary in `$PATH`.

Tests live alongside their source files in `#[cfg(test)] mod tests` blocks (the Rust convention), so they have access to private functions and `cargo test` builds them; the release binary contains no test code.

---

## Keyboard reference

Keys follow a **gradient**: a **bare letter** acts on the focused list, `Shift`
is the loud tier, `Ctrl` is global, `Alt` is app-wide commands (bytewarden uses
`0`–`4` for panel focus, so `Alt` is free for commands), and `/` focuses search.
On the **Search** box the letters are typed into the query, so its row actions
ride on `Alt+` instead.

```
GLOBAL (any screen)
  Ctrl+C quit · F1 help · F10 settings · Ctrl+P command palette
  Esc/h back · Tab cycle focus · 0..4 focus panel · / focus search

LOGIN
  Tab/Shift+Tab cycle field · ←→ Home End cursor · Space toggle checkbox
  F2 reveal master pwd · Enter login/unlock · Alt+K API-key · Alt+S SSO

VAULT — app commands (Alt, from any focus)
  Alt+S sync · Alt+E export · Alt+M import · Alt+W send · Alt+B memberships
  Alt+I fingerprint · Alt+G generator · Alt+L lock · Alt+O logout

VAULT — list [3]  (bare letters act on the highlighted row)
  j/k ↑↓ navigate · PgUp/PgDn page · Enter/l open detail
  n new · e edit · c copy password · u copy username
  f favorite ★ · x HIBP check · d delete · r restore (trash)
  (the same actions work from the Search box as Alt+letter)

VAULT — folders [1]
  j/k nav · Enter apply · n new · r rename · d delete

VAULT — items filter [2] · search [/]
  Items:  j/k nav · Enter apply
  Search: type to filter · ↑↓ PgUp/PgDn navigate · Enter open · Esc clear

DETAIL — read (bare letters act)
  j/k ↑↓ Tab fields · F2 reveal hidden · Esc/h back
  c copy field · e edit · m move to org · d delete · x HIBP
  a attach upload · s attach download · r restore (trash) · Alt+Del delete attachment

DETAIL — edit (Alt — it's a form, letters are typed)
  Tab/↑↓ field · ←→ Home End cursor · Enter save · Esc cancel · F2 reveal
  Alt+N add field · Alt+U add URL · Alt+T cycle type · Alt+R rename
  Alt+L assign collections · Alt+G generator · Alt+Del remove field

CREATE
  j/k Tab type-picker · Enter confirm/create · Esc cancel · F2 reveal
  Alt+G generator · Alt+L assign collections

GENERATOR
  Tab/↑↓ option · Space/←→ toggle/adjust · Enter generate
  Alt+C copy · Alt+U use in form · Esc close

HELP popup
  j/k ↑↓ PgUp/PgDn scroll · h/l ←→ pan · Home/End top/bottom · q/F1/Esc close

CONFIRMATIONS
  Enter confirm · D hard-delete (non-trash) · Esc/n cancel

MOUSE
  Wheel vertical · Shift+wheel horizontal · click focuses / selects
```

---

## License

See [`LICENSE`](LICENSE).
