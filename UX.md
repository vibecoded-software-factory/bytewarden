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
shared command log and hint bar.

- **Left sidebar** (content-sized, ~26 %): three stacked panels, each box
  hugging its rows so the column stays compact —
  - `─[0]-Status` — the feedback line (spinner / ✓ / ✗) **and the persistent
    `⚠ WORKER DEAD` condition badge** (`error`-bold; shown whenever the worker
    thread has died until restart, unlike the sticky error toast a keypress
    clears). Read-only chrome.
  - `─[1]-Folders` — "All folders", "(No folder)", a separator, then `📁`
    folders and `👥 Org / Collection` rows, each with an item count from the
    precomputed count maps.
  - `─[2]-Items` — the item-type filter list (All, Favorites, Login, Card,
    Identity, Note, SSH Key, separator, Trash) with per-row counts + icons.
- **Right main** (~74 %): `─[/]-Search` (3 rows) · `─[3]-Vault` list (fills) ·
  the `─[4]-Command log`.
- **Bottom**: the command log (responsive height) and the **per-focus hint
  bar** (`widgets::render_cmd_bar_with_help`: short hints on the left,
  **`F1 help` anchored right and never truncated**).

**Feedback lifetime.** Success toasts (`✓`) auto-clear after ~1.5 s; **error
toasts are sticky** — they persist until the next keypress clears them
(mutt/lazygit), because a failure is a condition to read, not a flash. The
`⚠ WORKER DEAD` badge is a separate *condition* indicator: it survives
keypresses and only goes away on restart.

There is **no nvim-style mode badge** — bytewarden navigates by focused panel
(`0`–`4` / `Tab`), not by editor modes.

Other screens (Login, Detail, Create, Generator) build their own `Layout` but
reuse the same chrome (`titled_block`, field cards, hint bar). Every popup
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
   symbol, `selected_bg` + bold row highlight, a tight `column_spacing(1)`,
   persisted scroll, and the automatic `draw_scrollbar` on overflow.
   Conventions:
   - **Reserve indicator columns per *view*, not per row.** Scan the visible
     (filtered) rows once; only reserve width for `★` (favorite), `🔒`
     (reprompt) or `👥` (org) if at least one visible row carries it. A
     personal-only / no-favorites view collapses the dead gutter so names
     start earlier.
   - **Size the type column to the widest `[label]`**, clamped 6–14 (not a
     fixed pad). The long names are abbreviated in the list so one rare long
     row can't pad every other: "Secure Note" → `[Note]`, "Identity" →
     `[Ident]`, "SSH Key" → `[SSH]`, so the column stays as narrow as
     `[Login]`. The detail screen shows the full type.
   - Type tag is colored per item type (`theme.item_login/card/identity/note/
     ssh`); selection (the `TableState`) indexes the **filtered** list.
   - **Only the final column may be `Constraint::Min`.** A stretching `Min` on
     a non-final column shoves trailing columns to the far right (the
     recurring "gap" bug).
2. **Sidebar lists** (folders, item-type filters) are Ratatui `List`s with a
   `▶ ` highlight symbol, a **`muted` dotted-rule separator row** (`┈`, via
   `separator_row`) injected before the last group — an explicit divider, not
   a blank gap — and a `ListState` whose visual index skips the separator.
   Per-row counts come from precomputed maps (`App::rebuild_sidebar_counts`) —
   never scan `items` per frame.

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

## Chrome & widgets (`view::widgets`)

**Border convention (hard rule).** **Every bordered surface is rounded**
(`BorderType::Rounded`, `Borders::ALL`) — section panels via
`widgets::titled_block`, popups / field cards / pickers via
`widgets::rounded_block` / `render_field_card`. Section panels carry their
identity through the numbered `─[N]-Name` title and the accent-when-focused
border; overlays add the centered `MODAL_*` band to say "overlay". Don't
hand-roll a `Block` with a different border type: a new panel is rounded via
`titled_block`, a new overlay/card via `rounded_block`.

- `widgets::titled_block(title, bottom, focused, theme)` — the rounded section
  panel: focused → accent border + **bold** title, else the `inactive` tint.
  `focused` is passed explicitly (the widget never reverse-engineers it from
  the color). Used for every panel.
- `widgets::focus_style(theme, focused)` — the **single** focused/unfocused
  chrome decision (`Theme::emphasis()` when focused, else `inactive`);
  `titled_block` and anything else that shows focus consume it.
- `widgets::rounded_block(style)` — the rounded frame for popups / field
  value boxes.
- `widgets::list_table` / `list_title` / `middle_ellipsis` /
  `trim_end_ellipsis` / `table_row_at` — the list renderer + sizing helpers.
- `widgets::draw_search_box(...)` — the `─[/]-Search` box; a leading `󰍉 `
  magnifying-glass affordance (one-column left margin, coloured to match the
  current state's text — not accent), then the placeholder when empty/unfocused
  or the block cursor when focused (via `editor_spans`).
- `widgets::editor_spans` / `editor_spans_masked` / `editor_lines` — the one
  text-input renderer over a `LineEditor` (masked `●` for secret fields —
  the login master password / OTP, detail hidden fields until reveal).
- `widgets::render_cmd_bar_with_help` — the bottom per-focus hint bar (hints
  left, `F1 help` anchored right); popups use the plain `render_cmd_bar`. The
  command log + `─[0]-Status` feedback panel are rendered inline in
  `view/vault.rs` (`render_cmd_log` / `render_status`).
- `widgets::draw_picker_modal(frame, theme, PickerModal { .. })` — **the**
  centered query/list overlay skeleton every picker renders through: `Clear`
  + rounded accent block + emphasized title (with its live count) + optional
  query row + a windowed list (multi-line items + non-selectable section
  headers supported) with the `▶` + `selected_bg` selection + a scrollbar +
  a width-fitted `legend_line` footer. A new picker overlay **must** use it.
- `widgets::draw_confirm_popup(frame, area, theme, ConfirmPopup { .. })` —
  **the** confirm overlay: a centered, rounded, error-bordered popup with the
  caller's body copy above one key-labelled `ConfirmAction` row per action
  (`Primary` accent · `Danger` error · `Cancel` dim), each row registered
  clickable (the mouse twin of its key) and the modal rect recorded for
  click-outside dismiss. Multi-action confirms (trash vs permanent delete)
  are extra `ConfirmAction` rows, never a bespoke popup. A new confirm is a
  `ConfirmPopup` value — never a new popup file.
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
- **Scroll registry** — every scrollable surface records its rect + a
  `widgets::ScrollTarget` as it draws (`register_scroll`, cleared each frame by
  `reset_scroll_regions`); `input::mouse` dispatches the wheel purely by pointer
  position (`scroll_target_at` → one `apply_scroll` table), so there is **no
  per-screen `match`** in the input layer. Adding a scrollable list is one
  `register_scroll` call at its draw site; the wheel handler never changes.
- **Click-outside-to-dismiss** — every centered overlay records its rect
  (`widgets::register_modal`, right where it computes its `center_rect` popup;
  cleared each frame). A `Down` outside the active modal (`active_modal_rect`)
  is the mouse twin of `Esc`: `input::mouse` routes a synthetic `Esc` through
  the active screen's own handler (`dispatch_screen_key`), so each overlay
  cancels the exact way its keyboard `Esc` does — no per-overlay close table.
- **Clickable chrome buttons** — a shared button registry
  (`widgets::register_button` / `button_at`, a `ClickAction` per rect, the same
  frame-local pattern as the scroll registry). The command bar's `F1 help` /
  `F10 settings` anchor is clickable — the mouse twin of the function keys.
- **Clickable overlay rows** — an overlay with internal navigation records a
  per-row hit map as it draws (same frame-local pattern; e.g.
  `view::settings::SettingsHit` / `settings_hit_at`) and its `input` module
  exposes a `mouse(app, col, row)` that maps the hit to the exact state its
  keyboard navigation would set. The **Settings** overlay is fully
  mouse-operable this way: click a sidebar section to select it, a panel row to
  focus it (a second click cycles/toggles its value), or a theme preset to
  preview it (a second click applies + saves) — the mouse twin of `↑/↓`,
  `←/→` and `Enter`.
- **Clickable form fields** — a form with a bespoke layout records each field's
  exact rect as it draws (same frame-local pattern; e.g.
  `view::login::login_field_at`, unioning a field's label row with its input
  box). The **Login** form focuses whatever field the pointer is over and
  toggles the checkbox rows (Save email / Auto-lock / Keep session) — read from
  the real layout rects, so the hit-testing can never drift from the renderer.
- **Right-click action menu** — right-clicking a vault row opens
  `Screen::ItemActions`, a compact centered menu of that item's secondary
  actions (open · copy username · copy password · copy TOTP · edit · move to
  collection · toggle favorite · delete; in the trash: open · restore · delete).
  Actions appear only when applicable — the copies only when the login carries
  that field, move only when the item is personal and can move into a single
  org's collection. The mouse layer seats the list
  cursor on the clicked row first, so every action runs against it through the
  ordinary `selected_item` path. Each action **delegates to the existing flow**
  (`flows::item_actions`), so the secret-exposing ones keep their reprompt gate —
  the menu can't bypass the master-password re-check. The menu itself is
  keyboard-navigable (`↑/↓` · `Enter` · `Esc`) and its rows are clickable
  (`view::item_actions::item_action_at`); one left-click runs an action. It
  closes on `Esc` or a click outside through the shared synthetic-`Esc` dismiss.
- `widgets::center_rect` / `MODAL_WIDTH_PCT` / `MODAL_HEIGHT` — the standard
  centered-modal geometry every list/picker/settings overlay imitates so they
  line up; `help_line`, the `favorite_star` emphasis, `key_style`,
  `Theme::emphasis()` / `Theme::danger_title()` — one definition each.
- Field cards (detail / edit / create) → `widgets::field_areas` /
  `field_areas_windowed` + `render_field_card`: a label row + a 3-row bordered
  value box, one card per `EditField`. `field_areas_windowed` windows the cards
  around the selected one so a field past the fold scrolls into view (the same
  windowing the preset picker uses); the Detail screen registers each visible
  card's rect (`view::detail::detail_field_at`) so a click focuses the card the
  pointer is over — a repeat click on the focused read-only card reveals it
  (through the same reprompt gate as F2), an edit-mode click focuses the field
  (reveal stays on the gated F2).

## Overlays & confirmations

Popups are centered and **always drawn over their base screen** by
`view::mod::draw`.

- **Confirmations** (`ConfirmDelete`, `ConfirmLogout`, `ConfirmDeleteFolder`,
  `ConfirmDeleteAttachment`) render through `draw_confirm_popup` — one
  confirm look in the whole app, `Esc` always the way out, worded so the
  consequence is clear. In a trash context the permanent-delete path is
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
- **AssignCollections** is mouse-operable: its rows are clickable — a click
  toggles that collection (the mouse twin of `Space`), read from the list's
  realised scroll offset (`view::assign_collections::collection_row_at`) so it
  stays correct when scrolled — and a `draw_scrollbar` rides a reserved
  right-column gutter when the collections overflow the panel.

## Command palette (`Ctrl+P`)

A `Ctrl+P` command palette (`Screen::CommandPalette`, `view::palette`) — a
fuzzy query (substring over the label) over a **context-aware** action list
(`flows::palette::palette_commands`: the app-wide verbs always, the item verbs
only with a selected non-trashed item), each row showing its keybinding
right-aligned so the palette doubles as an **executable cheat-sheet**. Opens
from the Vault or Detail; `↑↓` / `Ctrl+J`/`Ctrl+K` pick, `Enter` restores the
origin screen and runs the highlighted command — the *very same* `flows::*` the
keybinding would call (`PaletteCommand.run: fn(&mut App)`), so it can never
diverge. `Esc` / `Ctrl+P` cancel. Centered modal (`center_rect`, `Clear`,
rounded accent block); when the matches overflow the panel a `draw_scrollbar`
rides a reserved right-column gutter (so the track never clips a keybinding).

It is the **fifth** keybinding-sync surface: footer hints · `F1` help · README
tables · this file · **`flows::palette::palette_commands`** — keep all five in
sync on every keybinding / action change.

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

Every text input is a `domain::LineEditor` (UTF-8-safe char-index cursor;
readline word ops `Ctrl+W`/`Ctrl+U`/`Ctrl+←/→`/`Ctrl+A`/`Ctrl+E`
wired once in `route_line_editor` so every input inherits them; `ZeroizeOnDrop`
because any input can hold a secret). Detail / edit / create render one
`EditField` per field card. Conventions: `Tab`/`↑↓` move between fields, reveal
a hidden field (with the reprompt gate on protected items), generate a password
into the focused field, custom fields cycle type and rename via the input
popup, URL rows add a slot. Empty inputs show a dim placeholder
(`theme.placeholder`). Rendering is always `editor_spans` —
never a hand-rolled `char_indices().nth()` editor in a screen.

## Keybindings — the gradient convention (hard rule)

Keys are assigned by a **gradient of tiers**, so the modifier tells you the
weight of the action before you press it. Every screen follows the same tiers:

- **bare lowercase letter = the frequent, safe action on the focused list.**
  The vault `List`/`Folders` and the `Detail` read view don't type, so bare
  letters act on the highlighted row: on List `n` new · `e` edit · `c` copy
  password · `u` copy username · `f` favorite · `x` HIBP · `d` delete · `r`
  restore (trash); on Folders `n`/`r`/`d`; on Detail `c`/`e`/`m`/`d`/`x`/`a`/`s`/`r`.
  This is the lazygit / mutt model: a non-typing surface is "command mode".
  (`j`/`k`/`l`/`h` stay navigation, so no action binds them.)
- **`Shift+letter` = the loud tier.** Deleting is bare `d` — it always passes
  through the navigable confirm, which is the guard — and `Shift+D` (inside
  that confirm, non-trash) is the explicit permanent-delete.
- **`Ctrl` = global** — works from any focus, never confused with typed text:
  `Ctrl+C` quit (the **only** quit), `Ctrl+P` command palette,
  `Ctrl+W`/`Ctrl+U` word ops in every input.
- **`Alt+letter` = app-wide command.** bytewarden focuses panels with `0`–`4`
  (not `Alt`), so `Alt` is free for the vault-wide commands that fire from any
  focus: `Alt+S` sync · `Alt+E` export · `Alt+M` import · `Alt+W` send ·
  `Alt+B` memberships · `Alt+I` fingerprint · `Alt+G` generator · `Alt+L` lock ·
  `Alt+O` logout. On **typing surfaces** (the Search box, the edit/create
  forms) the row actions *also* park on `Alt+letter` so a text field can't trap
  you (`Alt+C` copy while searching; `Alt+G` generate into a field).
- **`/` = focus search** · `Esc`/`h` back · `F1` help · `F10` Settings ·
  `0`–`4` focus panel · `Tab` cycle · `j/k`+`↑/↓` navigate · `PgUp/PgDn` page ·
  `Enter`/`l` open. On the **Search** box `↑/↓` (not `j/k`, which type) move
  the list selection.

**Why the tiers.** A text field (Search, the edit forms) owns bare letters as
typed text; a non-typing surface (List, Folders, Detail-read) doesn't, so its
letters are free to act. Putting actions on bare letters there — and shifting
to `Alt` for app commands and typing-surface actions — is what makes the app
feel like a pro TUI instead of chord soup. The vim layer is a contract: the
`Esc` chain backs out one layer at a time (cancel field edit → leave edit mode
→ back) and **never destroys typed text**. During the transition the old
`Alt+letter` row shortcuts still work as aliases, but **bare is canonical**.
The footer shows only a few keys; the full per-screen list lives in the help
popup, the `README.md` tables and the command palette (`Ctrl+P`) — **keep all
five surfaces in sync** whenever a key changes.

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
`success`). **17 presets ship** (`Preset::ALL`, dark→light order; `nord` is
`Preset::DEFAULT`): the full Catppuccin family
(`-mocha`/`-frappe`/`-macchiato`/`-latte`), `dracula`, `nord`, `tokyonight`
(+ `-storm`), `gruvbox-dark`, `rose-pine` (+ `-dawn`), `everforest`,
`kanagawa`, `one-dark`, `solarized-dark`/`-light`, `monokai-pro`.
`name = "<preset>"` in `[theme]` picks the base and
per-key hex entries override it. The Settings picker (`F10`) applies live.
Adding a preset = one `Palette` arm in `Preset::palette`.

**Themes adapt to the terminal's color capability** at application time
(`theme::adapt` + `ColorCaps::detect`, never inside `from_palette` so palette
values stay exact): `NO_COLOR` collapses every hue to a grayscale tier
(brightness still differentiates; hue never carries meaning), a terminal
without a `COLORTERM=truecolor|24bit` hint gets every RGB quantized to the
nearest xterm-256 index (deterministic, controlled by us), and truecolor
passes through. `foreground: Reset` survives every mode.

## Settings overlay (`F10`)

`F10` opens a centered **Settings** overlay (`Screen::Settings`, drawn over
`settings_from`) — a left **section sidebar** + the active section's **panel**;
`Tab` switches sidebar ↔ panel, `↑/↓` move within, `←/→` change the focused
setting. Height/position follow the standard modal geometry; the width is
content-driven (sized once to the widest section so it doesn't resize as you
navigate) and clamped to the terminal. Values **wrap** onto continuation lines
rather than truncating, and the focused row's hint is pinned to the bottom.
Three sections today: **Theme** (a live preset picker whose list **scrolls** —
the presets window around the highlighted row and a `draw_scrollbar` cue rides
the panel's right border when they overflow), **Security** (auto-lock + its
window, keep-session, remember-email) and **Advanced** (clipboard-clear,
list-timeout). The non-Theme sections are value-lists: `↑/↓` pick a row, `←/→`
toggle a bool or step a number in place. Each row maps 1:1 to a `config.toml`
key. **Apply-immediately**: each change writes to the settings
cache *and* persists to `config.toml` on adjust (atomic write), so closing just
leaves. `Esc` steps back (Panel → Sidebar → close); `F10` closes from anywhere.

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

## Gaps vs this spec (known distance, shrink it — never grow it)

The code has not caught up with this document yet on:

- **Missing shared widgets** — `list_table` (lists/tables are still built
  inline per screen with `List`/`Table`), `draw_picker_modal` (the palette
  draws its own modal), `draw_input_popup` (input popups are hand-built per
  file).
- **The footer command bar** — `render_cmd_bar` still takes pre-built
  `full`/`short` hint strings; migrating the per-screen footers onto
  `legend_line`'s `(key, label)` pairs retires that duplication.
- **Edit-field cards aren't `LineEditor`s yet** — `EditField`
  (`tui/edit_field.rs`) tracks its own string + char cursor, so the
  detail/create cards render through the raw `cursor_line` core instead of
  `editor_spans`. Migrating `EditField` onto `domain::LineEditor` retires
  that seam.

When you touch an area listed here, close its gap in the same change (or a
dedicated PR) and delete its bullet.
