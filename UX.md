# UX.md — bytewarden UI/UX specification

The canonical design system for bytewarden's terminal UI. **Read this before
any UI change or new feature, and keep it up to date before every push** (see
`CLAUDE.md`). The whole point is that every screen looks and behaves the
same — when in doubt, reuse an existing component; never invent a one-off.

Everything documented here is backed by real code in `src/tui/view/` and
`src/tui/input/`. When you change a component, update this file in the same
change.

## Screen layout

The router `view::mod::draw` picks a renderer per `Screen`. The terminal floor
is **60×18** (`view::mod::is_terminal_too_small`); below it every screen is
replaced by the centered "terminal too small" notice (`view::mod::draw_too_small`):
three vertically-centered lines — an `error`-colored bold `Terminal too small`
title, a `dim` `Resize to at least {W}×{H} (currently {w}×{h})` line, and a `dim`
`Ctrl+C to quit` hint.

**Vault** (`view/vault.rs`) is the reference layout: a 2-column body over a
bottom hint bar.

- **Left sidebar** (26 %): three stacked panels —
  - `─[0]-Status` — the feedback line (spinner / ✓ / ✕), read-only.
  - `─[1]-Folders` — "All folders", "(No folder)", a separator, then `📁`
    folders and `👥 Org / Collection` rows, each with an item count.
  - `─[2]-Items` — the item-type filter list (All, Favorites, Login, Card,
    Identity, Note, SSH Key, separator, Trash) with per-row counts + icons.
- **Right main** (74 %): `─[/]-Search` (3 rows) · `─[3]-Vault` list (fills) ·
  `─[4]-Command Log` (grows 4→9 rows once there are entries).
- **Bottom**: the per-focus hint bar with `F1: help` anchored right.

Other screens (Login, Detail, Create) build their own `Layout` but reuse the
same chrome (`titled_block`, field cards, hint bar). Every popup screen draws
its **origin screen underneath** and overlays a centered popup — the router
does this explicitly (e.g. `ConfirmDelete` draws `vault` then the popup;
`AttachmentUpload` draws `detail` then the popup).

## Panels & focus

The vault's focusable panels are the `screens::Focus` variants: `Status`,
`Search`, `Folders`, `Items`, `List`, `CmdLog`. Conventions:

- Number keys `0`–`4` jump straight to a panel (`App::focus_panel`); `Tab`
  cycles (`App::cycle_focus`); `/` jumps to Search.
- A panel is "focused" → accent **square** border + **bold** title
  (`widgets::focus_color` / `focus_border` + `titled_block`); otherwise the
  `inactive` tint.
- Panel titles follow the **`─[N]-Name`** form (the `[N]` doubles as the
  number-key focus target) with a dim `{selected} of {total}` counter in the
  bottom-right (`widgets::titled_block`).
- Sidebar panels are **content-sized** (each box hugs its rows; leftover height
  is an empty filler at the bottom) so the column stays compact instead of
  stretching one panel to fill.

## Lists & tables

bytewarden has **two list flavors** — there is no single `list_table` helper;
reuse the matching pattern:

1. **The vault item list** (`render_list`) is a Ratatui **`Table`** with two
   columns: an indicators+type cell and the name. Follow its conventions:
   - **Reserve indicator columns per *view*, not per row.** Scan the visible
     (filtered) rows once; only reserve width for `★` (favorite), `🔒`
     (reprompt) or `👥` (org) if at least one visible row carries it. A
     personal-only / no-favorites view collapses the dead gutter so names
     start earlier.
   - **Size the type column to the widest visible `[label]`** (a `col_width`
     pass over the visible rows), clamped to 6–14, instead of a fixed pad. "Secure Note"
     is shortened to `[Note]` in the list (`list_type_label`); the detail
     screen shows the full type.
   - Type tag is colored per item type (`theme.item_login/card/identity/note/
     ssh`); `column_spacing(2)`; row highlight = `selected_bg` + bold;
     `highlight_symbol("▶ ")`; selection (`TableState`) indexes the **filtered**
     list.
2. **Sidebar lists** (folders, item-type filters) are Ratatui **`List`s** with
   a `▶ ` highlight symbol, a `muted` separator row injected before the last
   group, and a `ListState` whose visual index skips the separator. Per-row
   counts come from precomputed maps (`App::rebuild_sidebar_counts`) — never
   scan `items` per frame.

Row indicators (item list): `★` favorite, `🔒` reprompt-protected, `👥`
belongs to an organisation.

## List state convention (`App`)

The vault list keeps: `filtered_cache: Vec<usize>` (indices into `items` or
`trashed_items` surviving the active type-filter + folder-filter + search),
`search_query: String`, `selected_index` (indexes the **filtered** cache),
`scroll_offset`. Rebuild via `rebuild_filtered_cache()` (or `rebuild_caches()`
after a wholesale items change, which also refreshes `items_lowered` and the
sidebar counts). Ranking is `domain::search::fuzzy_score_lowered` over the
pre-lowercased `LoweredItem` projection; the pure core is
`app::compute_filtered_indices`. The **`url:`** query prefix narrows to login
URIs (substring, no fuzzy reorder). Selection always indexes the filtered
cache, never the raw vec.

## Chrome & widgets (`view::widgets`)

- `titled_block(title, bottom, col, theme)` / `rounded_block(style)` — the
  bordered block (rounded). Used for every panel.
- `focus_color(focused, accent, inactive)` / `focus_border(focused, accent)` —
  the focus tint helpers; don't branch on focus by hand.
- `render_cmd_bar_with_help(...)` — the bottom hint bar for **main screens**
  (Login, Vault, Detail, Create): picks the longest of `full`/`short` hints
  that fits, with **`F1: help` anchored right and never truncated**.
  `render_cmd_bar(...)` is the same minus the anchor — for **popups**, which
  carry their own self-contained instructions and don't open F1.
- `input_with_cursor(text, cursor, focused, theme)` / `cursor_line(...)` — the
  one-line text input renderer with a reverse-video `█` block cursor at the
  char index. The single text-input renderer (search, login fields, popup
  inputs).
- `render_checkbox(...)` — the `☐` / `☑` labelled checkbox.
- `field_areas(count, area)` + `render_field_card(...)` — the 4-row field card
  (1 label row + 3-row bordered value box) used by detail / edit / create.
- `center_rect(width_pct, height, area)` — centered sub-rect for popups.
- `help_line(key, desc, theme)` — one `key  description` row for the help
  popup.

## Overlays & confirmations

Popups are centered (double-border for the help popup; `center_rect` chrome
elsewhere) and **always drawn over their origin screen** by `view::mod::draw`,
so they feel overlaid in context (e.g. the reprompt and assign-collections
popups read their stored `origin` to pick which screen to draw underneath).

- **Confirmations** (`ConfirmDelete`, `ConfirmLogout`, `ConfirmDeleteFolder`,
  `ConfirmDeleteAttachment`): `Enter` confirms, `Esc`/`n` cancels. In a trash
  context `D` (uppercase) is the explicit hard-delete shortcut. Destructive
  ops are worded so the consequence is clear before committing.
- **Form popups** (Export, Import, SendCreate, FolderName, RenameField,
  AttachmentUpload/Download, AssignCollections, RepromptUnlock): self-contained
  instructions via `render_cmd_bar`, `Esc` cancels. **F1 does not open help
  from a popup** — `input::mod::f1_opens_help` returns `true` only on Vault,
  Login, Detail and Create; the user must `Esc` out first.

## Help popup

`view::help::draw_popup` is **context-aware**: it shows only the keys for the
originating screen (`App::help_from`) and, on the vault, narrows further to the
focused panel (`App::focus`). The renderer is the source of truth for the
viewport — it clamps `App::help_scroll` against the real content overflow, so
the input layer (`input::mod::handle_help`) can bump the offset freely
(`j/k`/arrows scroll, `h/l` + `Shift+H/L` pan by `HELP_PAGE_COLS`, `PgUp/PgDn`
by `HELP_PAGE_ROWS`, `Home/End`, `q`/`F1`/`Esc` close). Add a section +
`screen_label` entry for every new screen.

## Inline editing & forms

Detail / edit / create render one `EditField` (`tui/edit_field.rs`) per field
card (`render_field_card`). Conventions: `Tab`/`↑↓` move between fields, `F2`
reveals a hidden field, `Alt+G` generates a password into the focused field,
custom fields cycle type with `Alt+T` and rename via the RenameField popup,
URL rows add with `Alt+U`. Empty inputs show a dim placeholder
(`theme.placeholder`).

## Keybindings (global conventions)

- `/` focus search · `Esc`/`h` back · `F1`/`?` help · `F9` Settings ·
  `0`–`4` focus panel · `Tab` cycle focus · **only `Ctrl+C` quits**
  (everything else is free for navigation / type-to-search).
- `j/k` + `↑/↓` navigate · `PgUp/PgDn` page · `Enter`/`l` open.
- **Actions use the `Alt+<letter>` convention** (distinctive to bytewarden):
  e.g. `Alt+C` copy password, `Alt+U` copy username, `Alt+N` new, `Alt+E`
  edit, `Alt+S` sync, `Alt+D` delete, `Alt+F` favorite, `Alt+G` generator,
  `Alt+X` HIBP check. `is_alt` accepts any modifier set containing `ALT` so
  AltGr keyboards work. The footer shows only a few; the full per-screen list
  lives in the help popup and the `README.md` tables — **keep both in sync**.

## Theme (`tui::theme`)

Resolved from the optional `[theme]` section of `config.toml`; every key
optional, partial configs valid. Fields: `accent` (active/highlights),
`inactive` (unfocused borders/titles), `selected_bg` (row highlight),
`success`, `error`, `dim` (secondary text/counters), `foreground` (main text;
defaults to `Color::Reset` to inherit the terminal), `placeholder`, `muted`
(separators/barely-visible borders), `star_dim`/`star_mid`/`star_bright`
(splash/login background), and the item-type colors `item_login`/`item_card`/
`item_identity`/`item_note`/`item_ssh`/`item_favorite`. **Don't hardcode
colors — use these.**

**Presets.** Themes are built from a `Palette` (13 named roles) via
`Theme::from_palette`, which maps the core roles and derives the starfield +
item-type colors (`item_note` maps to the teal `cyan` role so it stays distinct
from the green `success`). Four presets ship (`Preset::ALL`: `catppuccin-mocha`,
`dracula`, `nord` (default — `Preset::DEFAULT`), `catppuccin-latte`);
`name = "<preset>"` in `[theme]` picks the base and per-key hex entries override
it. The Settings picker (`F9`) applies live. Adding a preset = one `Palette` arm
in `Preset::palette`.

## Settings overlay (`F9`)

`F9` opens a centered **Settings** overlay (`Screen::Settings`, drawn over
`settings_from`) — `view::settings::draw_popup`, input in `input::settings`.
Layout: a left **section sidebar** + the active section's **panel**,
`Tab`/arrows move between and within them. It's **sectioned so the preferences
surface can grow** (Security, Clipboard…) without changing the chrome; today the
only section is **Theme**, a preset picker that **previews live** as you move
(`App::settings_preview_theme`) — `Enter` applies + persists `name = "<preset>"`
to `config.toml` (`SettingsPort::write_theme_name`), `Esc`/`F9` cancels and
restores the pre-open theme. The bottom hint bar anchors `F1: help · F9:
settings`. Add a section by extending `SettingsSection`.

## Branding

The figlet wordmark (`view::logo`, bundled `slant.flf`) over the decorative
`view::starfield` belongs to the **splash and login** screens only; never
repeat the "bytewarden" name as decorative text on working screens. Screen
identity comes from the bordered `─[N]-Name` block titles.

## Golden rules

1. **Reuse, don't reinvent** — a new panel = `titled_block`; a new input =
   `input_with_cursor`; a new field = `render_field_card`; a new popup =
   `center_rect` + `render_cmd_bar`; a new bottom bar = `render_cmd_bar_with_help`.
2. **Fix the class, not the instance** — when you change one screen, change
   every screen with the same pattern (and update this file).
3. **Every change stays coherent** with the rest of the UI. If you're tempted
   to diverge, update the spec here first and apply it everywhere.
