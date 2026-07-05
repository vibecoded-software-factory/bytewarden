# UX.md — bytewarden UI/UX specification

The canonical design system for bytewarden's terminal UI. **Read this before
any UI change or new feature, and keep it up to date before every push** (see
`CLAUDE.md`). The whole point is that every screen looks and behaves the
same — when in doubt, reuse an existing component; never invent a one-off.

> **This spec is the target of an in-progress restructuring.** It describes
> where the UI is going. Where the code diverges, **the spec wins** — bring
> the code to it, and update the spec in the same change when a decision here
> changes. Everything documented here is backed (or to be backed) by real
> code in `src/tui/view/` and `src/tui/input/`.

**Responsiveness is a hard rule.** Every element must adapt to the panel
width: show in full when it fits, otherwise **wrap** onto continuation lines
(action bars, field values) or **trim to the width** with a trailing `…`
(one-line previews — item names, snippets — where only the start matters) —
never a fixed character cap that the terminal then clips. Size against the
real content width, not a magic number.

## Screen layout

The router `view::mod::draw` picks a base screen per `Screen`, then overlays
any popup on top (popups draw the base screen underneath first). The terminal
floor is **60×18** (`view::mod::MIN_W`/`MIN_H`); below it every screen is
replaced by the centered "terminal too small" notice
(`view::mod::draw_too_small`): three vertically-centered lines — an
`error`-colored bold `Terminal too small` title, a `dim` `Resize to at least
{MIN_W}×{MIN_H} (currently {w}×{h})` line, and a `dim` `Ctrl+C to quit` hint.

**Vault** (`view/vault.rs`) is the reference layout: a 2-column body over the
shared command log and status strip.

- **Left sidebar** (content-sized, ~26 %): three stacked panels, each box
  hugging its rows so the column stays compact —
  - `─[0]-Status` — the feedback line (spinner / ✓ / ✗), read-only chrome.
  - `─[1]-Folders` — "All folders", "(No folder)", a separator, then `📁`
    folders and `👥 Org / Collection` rows, each with an item count from the
    precomputed count maps.
  - `─[2]-Items` — the item-type filter list (All, Favorites, Login, Card,
    Identity, Note, SSH Key, separator, Trash) with per-row counts + icons.
- **Right main** (~74 %): `─[/]-Search` (3 rows) · `─[3]-Vault` list (fills) ·
  the `─[4]-Command log`.
- **Bottom**: the command log (responsive height) and the **status strip**
  (mode badge · condition badges · feedback / per-focus hint · `F1 help`
  anchored right).

Other screens (Login, Detail, Create, Generator) build their own `Layout` but
reuse the same chrome (`titled_block`, field cards, status strip). Every popup
screen draws its **origin screen underneath** and overlays a centered popup —
the router does this explicitly (e.g. `ConfirmDelete` draws `vault` then the
popup; `AttachmentUpload` draws `detail` then the popup; the reprompt and
assign-collections popups read their captured `origin`).

## Boot & loading

The **splash doubles as the loading screen** — the app never enters the vault
half-loaded. The boot sequence (`flows::auth`): `request_status` shows the
splash + spinner with **"Checking session…"**; on an unlocked/resumable
session the splash *stays up* and the legend flips to **"Loading vault…"**
while the item `list` runs; only when it lands does the handler transition
**Splash → Vault** with the items already in hand. Locked → Login (unlock);
logged out → Login (full login). A failed boot still surfaces its error with a
retry affordance rather than stranding on a dead splash. There is **no loading
skeleton** — the splash legend is the single loading affordance.

## Panels & focus

The vault's focusable panels are the `screens::Focus` variants: `Status`,
`Search`, `Folders`, `Items`, `List`, `CmdLog`. Conventions:

- **Every focusable panel's border tag is its literal go-to combo.** Titles
  follow the **`─[N]-Name`** form where `[N]` is the number-key focus target
  (`0`–`4`); `/` jumps to Search. `Tab`/`Shift+Tab` cycle focus via
  `input::common::cycle_focus`.
- **Border tiers signal reachability** (not just two states):
  - **focused** → accent + bold (`view::mod::titled_block(title, true, app)`).
  - **available, unfocused** → the `inactive` tint (a *bright*, near-text
    gray — you can Tab / go-to it).
  - **unavailable** → the darker `muted` tint for a panel you can't focus
    right now, so it doesn't read as a Tab target that's ignoring you.
- A dim `{selected} of {total}` counter sits bottom-right in the border
  (`widgets::list_title` → `list_table`).

## Lists & tables — the single pattern

bytewarden has **two list flavors**; reuse the matching one, never a one-off.

1. **The vault item list** renders through **`widgets::list_table`** — a
   Ratatui `Table` with an indicators+type cell and the name. It owns the
   bordered focus-styled block, the dim+bold header, the `▶ ` selection
   symbol, `selected_bg` + bold row highlight, `column_spacing(2)`, persisted
   scroll, and the automatic `draw_scrollbar` on overflow. Conventions:
   - **Reserve indicator columns per *view*, not per row.** Scan the visible
     (filtered) rows once; only reserve width for `★` (favorite), `🔒`
     (reprompt) or `👥` (org) if at least one visible row carries it. A
     personal-only / no-favorites view collapses the dead gutter so names
     start earlier.
   - **Size the type column to the widest visible `[label]`**, clamped 6–14
     (not a fixed pad). "Secure Note" is shortened to `[Note]` in the list;
     the detail screen shows the full type.
   - Type tag is colored per item type (`theme.item_login/card/identity/note/
     ssh`); selection (the `TableState`) indexes the **filtered** list.
   - **Only the final column may be `Constraint::Min`.** A stretching `Min` on
     a non-final column shoves trailing columns to the far right (the
     recurring "gap" bug).
2. **Sidebar lists** (folders, item-type filters) are Ratatui `List`s with a
   `▶ ` highlight symbol, a `muted` separator row injected before the last
   group, and a `ListState` whose visual index skips the separator. Per-row
   counts come from precomputed maps (`App::rebuild_sidebar_counts`) — never
   scan `items` per frame.

Row indicators (item list): `★` favorite, `🔒` reprompt-protected, `👥`
belongs to an organisation.

## List state convention (`App`)

The vault list keeps: `filtered_cache: Vec<usize>` (indices into `items` or
`trashed_items` surviving the active type-filter + folder-filter + search),
`search_query` (backed by a `LineEditor`), `selected_index` (indexes the
**filtered** cache), `scroll_offset`. Rebuild via `rebuild_filtered_cache()`
(or `rebuild_caches()` after a wholesale items change). Ranking is
`domain::search::fuzzy_score_lowered` over the pre-lowercased `LoweredItem`
projection; the pure core is `app::compute_filtered_indices`. The **`url:`**
query prefix narrows to login URIs (substring, no fuzzy reorder). Selection
always indexes the filtered cache and is re-anchored **by id** after a reload,
never left dangling by index. See `CLAUDE.md` → *State & invalidation
contracts* for the full rebuild table.

## Chrome & widgets (`view::widgets` + `view::mod`)

**Rounded borders everywhere (hard rule).** Every section panel and popup uses
`BorderType::Rounded`, set once in `titled_block` / `list_table` / the picker
chrome — position (the `MODAL_*` band) and the title style say "overlay", not
a heavier border.

- `view::mod::titled_block(title, focused, app)` / `disabled_block(title,
  app)` — the bordered rounded panel (focused = accent+bold, available =
  `inactive`, unavailable = `muted`). Used for every panel.
- `widgets::focus_style(theme, focused)` — the **single** focused/unfocused
  chrome decision; everything that shows focus consumes it.
- `widgets::list_table` / `list_title` / `middle_ellipsis` /
  `trim_end_ellipsis` / `table_row_at` — the list renderer + sizing helpers.
- `widgets::draw_search_box(...)` — the `─[/]-Search` box; placeholder when
  empty/unfocused, block cursor when focused (via `editor_spans`).
- `widgets::editor_spans` / `editor_spans_masked` / `editor_lines` — the one
  text-input renderer over a `LineEditor` (masked `●` for secret fields —
  the login master password / OTP, detail hidden fields until reveal).
- `widgets::draw_cmd_log` / `widgets::draw_status_strip` — the command-log
  panel and the bottom feedback/hint strip.
- `widgets::draw_picker_modal(frame, theme, PickerModal { .. })` — **the**
  centered query/list overlay skeleton every picker renders through: `Clear`
  + rounded accent block + emphasized title (with its live count) + optional
  query row + a windowed list (multi-line items + non-selectable section
  headers supported) with the `▶` + `selected_bg` selection + a scrollbar +
  a width-fitted `legend_line` footer. A new picker overlay **must** use it.
- `widgets::draw_confirm_popup(frame, area, theme, title, body, confirmed)` +
  `input::common::run_confirm` — **the** navigable y/n overlay (built on the
  picker skeleton). `←/→` (or `Tab`/`h`/`l`) move between confirm/cancel,
  `Enter` activates the highlighted one, `y`/`n`/`Esc` are shortcuts.
  **Default highlight = cancel** for destructive actions.
- `widgets::draw_input_popup(...)` — the small centered single-input popup
  (folder name, rename field, attachment path, new-conversation-style boxes).
- `widgets::legend_line(&[(key, label)], width, theme)` — **the** hint/legend
  builder: keys through `key_style` (accent + bold), labels dim, ` · `
  separators, fitted to the width by whole segments — **never a clipped key**.
  Every footer hint and inline legend routes here.
- `widgets::button(label, active, theme)` — **the** `[ label ]` action button
  (login buttons, confirm confirm/cancel). One look for every button.
- `widgets::empty_state_lines(head, hints, theme)` — the shared empty-state
  body (bold headline + dim hints). **Every empty state teaches**: it names
  the 2-3 keys that would fill the panel (an empty vault → `Alt+N to create`;
  a search that matches nothing → `Esc clears the filter`; a failed load →
  the error + a retry key). A bare dim line is not an acceptable empty state.
- `widgets::draw_scrollbar` — one dim right-border scrollbar on **every**
  overflowing region (list, picker list, command log, help).
- `widgets::center_rect` / `MODAL_WIDTH_PCT` / `MODAL_HEIGHT` — the standard
  centered-modal geometry every list/picker/settings overlay imitates so they
  line up; `help_line`, `unread`/`favorite_star` emphasis, `key_style`,
  `Theme::emphasis()` / `Theme::danger_title()` — one definition each.
- Field cards (detail / edit / create) → `widgets::field_areas` +
  `render_field_card`: a label row + a 3-row bordered value box, one card per
  `EditField`.

## Overlays & confirmations

Popups are centered and **always drawn over their base screen** by
`view::mod::draw`.

- **Confirmations** (`ConfirmDelete`, `ConfirmLogout`, `ConfirmDeleteFolder`,
  `ConfirmDeleteAttachment`) render through `draw_confirm_popup` /
  `run_confirm` — one confirm look in the whole app, default = cancel, worded
  so the consequence is clear. In a trash context the permanent-delete path is
  an explicit second confirm/shortcut; the destructive item op stays a bare
  key (`x` / `Alt+D`) *because* it always passes through this confirm.
- **Form popups** (Export, Import, SendCreate, FolderName, RenameField,
  AttachmentUpload/Download, AssignCollections, RepromptUnlock): self-contained
  instructions via `legend_line`; keys route through
  `input::common::route_line_editor`; `Esc` cancels. **F1 does not open help
  from a popup** — the user must `Esc` out first. Import's format field is a
  `◀ X ▶ (n of m)` dropdown; Export's format cycles with a format-dependent
  security note (error-colored for plaintext CSV/JSON, dim for encrypted).
- **Memberships** (`Memberships`) is a read-only picker: organisations with
  their collections, pre-sorted; `Esc`/`Enter`/`q` close.

## Command palette (rewrite target)

A `Ctrl+P` command palette (`Screen::CommandPalette`) is a target of the
restructuring — a fuzzy query over a **context-aware** action list
(`flows::palette::palette_commands`: only the actions valid where you are),
each row showing its keybinding right-aligned so the palette doubles as an
executable cheat-sheet. It shares the `draw_picker_modal` skeleton and calls
the very same `flows::*` the keybinding would. When added, it becomes the
**fifth** keybinding-sync surface (footer · `F1` help · README tables · this
file · `palette_commands`) — keep all five in sync on every keybinding change.

## Help popup

`view::help::draw` is **context-aware and scrollable**: it shows the shortcuts
for the screen it was opened from (`App::help_from`, stamped on F1) plus a
shared Global section, and on the vault it narrows further to the focused
panel. The renderer owns the viewport — it clamps `App::help_scroll` against
the real overflow, so the input handler bumps the offset freely (`j/k`/arrows
scroll, `PgUp/PgDn` page, `g/G` top/bottom, `q`/`Esc`/`F1` close); `▲`/`▼`
border marks flag hidden content. Add a section for every new screen and keep
it in sync with `README.md`.

## Inline editing & the text-input model

Every text input is a `domain::LineEditor` (UTF-8-safe byte cursor on a char
boundary; readline word ops `Ctrl+W`/`Ctrl+U`/`Ctrl+←/→`/`Ctrl+A`/`Ctrl+E`
wired once in `route_line_editor` so every input inherits them; `ZeroizeOnDrop`
because any input can hold a secret). Detail / edit / create render one
`EditField` per field card. Conventions: `Tab`/`↑↓` move between fields, reveal
a hidden field (with the reprompt gate on protected items), generate a password
into the focused field, custom fields cycle type and rename via the input
popup, URL rows add a slot. Empty inputs show a dim placeholder
(`theme.placeholder`). Rendering is always `editor_spans` / `editor_lines` —
never a hand-rolled `char_indices().nth()` editor in a screen.

## Keybindings — the gradient convention (hard rule)

Keys are assigned by a **gradient of tiers**, so the modifier tells you the
weight of the action before you press it. Every screen follows the same tiers:

- **bare lowercase letter = the frequent, safe action on the focused list.**
  On the vault `List`/`Folders`/`Items` panels (which don't type) bare letters
  act on the cursor row (`n` new, `e` edit, `c` copy, `f` favorite, `r`
  refresh/restore, `g` generator, …). This is the lazygit / mutt model: a list
  is "command mode".
- **`Shift+letter` = the loud tier.** Deleting a concrete item is bare `x`
  (it always passes through the navigable confirm — that's the guard);
  `Shift+D` is the explicit permanent-delete in trash. Loud one-keystroke
  status ops live here.
- **`Ctrl` = global** — works from any focus, never confused with typed text:
  `Ctrl+C` quit (the **only** quit), `Ctrl+P` command palette (target),
  `Ctrl+D`/`Ctrl+U` half-page in every list, `Ctrl+W`/`Ctrl+U` word ops in
  every input.
- **`Alt+letter` = jump to a panel** (matching its border tag) **+
  compose-context verbs** — actions on a screen whose bare letters are typed
  text (the Search box, the edit/create forms) park on `Alt` so a text field
  can't trap you (e.g. `Alt+G` generate into the focused field).
- **`/` = focus search** · `Esc`/`h` back · `F1` help · `F9` Settings ·
  `0`–`4` focus panel · `Tab` cycle · `j/k`+`↑/↓` navigate · `PgUp/PgDn` page ·
  `g/G` top/bottom · `Enter`/`l` open.

**Why the tiers.** A text field (Search, the edit forms) owns bare letters as
typed text; a list doesn't type, so its letters are free to act. Putting
actions on bare letters in lists — and only shifting to `Alt`/`Ctrl` where
text input would collide — is what makes the app feel like a pro TUI instead
of chord soup. The vim layer is a contract: the `Esc` chain backs out one
layer at a time (cancel field edit → leave edit mode → back) and **never
destroys typed text**. The footer shows only a few keys; the full per-screen
list lives in the help popup, the `README.md` tables and (when added) the
command palette — **keep all in sync** whenever a key changes.

## Theme (`tui::theme`)

Resolved from the optional `[theme]` section of `config.toml`; every key
optional, partial configs valid. Fields: `accent` (active/highlights),
`inactive` (unfocused borders — a *bright*, near-text gray), `selected_bg`
(row highlight), `success`, `error`, `dim` (readable secondary text), `muted`
(separators / barely-visible chrome), `foreground` (main text; defaults to
`Color::Reset` to inherit the terminal), `placeholder`, the splash starfield
tiers `star_dim`/`star_mid`/`star_bright`, and the item-type colors
`item_login`/`item_card`/`item_identity`/`item_note`/`item_ssh`/
`item_favorite`. **Don't hardcode colors — use these.**

**Legibility hierarchy (hard rule).** De-emphasis comes from *hierarchy*,
never from painting content almost the colour of the border. Pick by what the
text **is**:

1. **`foreground`** — primary content (item names, field values, the command
   being run, input text). The thing the user is reading.
2. **`accent`** (often `+ BOLD`) — emphasis / interaction: the focused-panel
   border + title, the `▶` cursor, keybind letters, the `★` favorite.
3. **`dim`** — *readable* secondary text: counters (`· X of Y`), the
   command-log detail, footer hints, the column-header row. A subtext that
   stays legible — **not** the border tint.
4. **`inactive`** — unfocused panel **borders** only. A bright, near-text gray;
   what marks focus is the *active* border going accent+bold, never the
   inactive one fading out.
5. **Recessive band** — genuinely faint, chrome only: `placeholder`
   (empty-input hints), `muted` (` · ` separators, disabled chips). Never put
   content a user must read here.

**Navigable list rows are content, not chrome** — sidebar rows, item rows,
picker rows render at `foreground` (the selected one `+ bold`, accent+bold
when its pane is focused); the `▶` + accent carry the selection, so unselected
rows must **not** be `dim`. When in doubt, one tier brighter.

**Presets.** Themes are built from a `Palette` (13 named roles) via
`Theme::from_palette`, which maps the core roles and *derives* the rest:
`inactive`/`dim` are lifted out of the dark band toward text (blends, not the
border tint), the starfield tiers fade from accent, and the item-type colors
map to distinct hues (`item_note` stays teal so it never reads as the green
`success`). Multiple presets ship (`Preset::ALL`, dark→light order; `Nord` is
`Preset::DEFAULT`); `name = "<preset>"` in `[theme]` picks the base and
per-key hex entries override it. The Settings picker (`F9`) applies live.
Adding a preset = one `Palette` arm in `Preset::palette`.

**Themes adapt to the terminal's color capability** at application time
(`theme::adapt` + `ColorCaps::detect`, never inside `from_palette` so palette
values stay exact): `NO_COLOR` collapses every hue to a grayscale tier
(brightness still differentiates; hue never carries meaning), a terminal
without a `COLORTERM=truecolor|24bit` hint gets every RGB quantized to the
nearest xterm-256 index (deterministic, controlled by us), and truecolor
passes through. `foreground: Reset` survives every mode.

## Settings overlay (`F9`)

`F9` opens a centered **Settings** overlay (`Screen::Settings`, drawn over
`settings_from`) — a left **section sidebar** + the active section's **panel**;
`Tab` switches sidebar ↔ panel, `↑/↓` move within, `←/→` change the focused
setting. Height/position follow the standard modal geometry; the width is
content-driven (sized once to the widest section so it doesn't resize as you
navigate) and clamped to the terminal. Values **wrap** onto continuation lines
rather than truncating, and the focused row's hint is pinned to the bottom.
Sectioned so the surface can grow (Theme, Clipboard, Security…); today Theme is
a live preset picker. **Apply-immediately**: each change writes to the settings
cache *and* persists to `config.toml` on adjust (atomic write), so closing just
leaves. `Esc` steps back (Panel → Sidebar → close); `F9` closes from anywhere.

## Responsiveness — what adapts (and how)

Responsiveness is a hard rule (see the top of this file). Reuse these
mechanisms; don't regress them:

- **Command log is height-responsive** (`widgets::cmdlog_height`): it yields
  rows to the list as the terminal gets short (monotonically — a taller
  terminal never shrinks the body); `0` hides it.
- **Footer hint** fits by whole ` · ` segments (`legend_line` / `fit_segments`)
  with a trailing ` …`, never cutting a keybinding; `F1 help` is anchored
  right and reserved first.
- **List columns** size to the *visible* rows, clamped; only the final column
  may be `Min`. Overflow uses `middle_ellipsis` (head-biased, keeps an
  identifier tail) or `trim_end_ellipsis` (start-anchored previews).
- **Field values** wrap onto continuation lines rather than clipping.
- **Every overflowing region draws a scrollbar** (`draw_scrollbar`); short
  lists stay clean. The list, command log and help each own their viewport and
  clamp scroll against the *real* overflow (so a `usize::MAX` "jump to end"
  sentinel is safe).
- **Modals share one geometry** (`center_rect(MODAL_WIDTH_PCT, MODAL_HEIGHT)`)
  and window their content by the *real* inner height, so a short-but-valid
  terminal shrinks the modal and the content follows — nothing clips off the
  bottom.
- **Resize hygiene**: `view::draw` stamps `mouse_areas` with the frame size
  each frame and the run loop `terminal.clear()`s on a size change; clicks
  whose coordinates predate the latest resize are dropped.

## Branding

The figlet wordmark (`view::logo`, bundled `slant.flf`) over the decorative
`view::starfield` belongs to the **splash and login** screens only; never
repeat the "bytewarden" name as decorative text on working screens. Screen
identity comes from the bordered `─[N]-Name` block titles.

## Golden rules

1. **Reuse, don't reinvent** — a new list = `list_table`; a new input =
   `LineEditor` + `editor_spans` (routed via `input::common`); a new panel =
   `titled_block`; a new confirm = `draw_confirm_popup`; a new picker =
   `draw_picker_modal`; a new button = `widgets::button`; a new hint =
   `legend_line`; a new empty state = `empty_state_lines`.
2. **Fix the class, not the instance** — when you change one screen, change
   every screen with the same pattern (and update this file).
3. **Every change stays coherent** with the rest of the UI. If you're tempted
   to diverge, update the spec here first and apply it everywhere.
4. **No decorative noise on working screens** — the figlet + starfield belong
   to splash/login only. Screen identity comes from the bordered block titles.
