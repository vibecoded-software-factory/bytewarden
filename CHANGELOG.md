# Changelog

All notable changes to this project will be documented in this file.

The format is loosely based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Performance & reliability fixes

- **Pipe-buffer deadlock en `wait_with_timeout` resuelto** — el helper
  drenaba `stdout`/`stderr` *después* de que el child saliera. Con
  outputs > 64 KB (el pipe buffer típico en Linux), `bw list items`
  quedaba bloqueado escribiendo, nosotros polleabamos vacíamente, y la
  única salida era el timeout. Resultado visible: vaults grandes nunca
  cargaban tras el login y la TUI mostraba "timed out after 10s".
  Ahora reader threads paralelos drenan ambos pipes mientras el child
  corre — mismo patrón que `Child::wait_with_output` internamente.
  Regression test agregado con 256 KB de output.
- **`Stdio::null()` en spawns de bw** — `bw_run_timeout` y
  `bw_run_with_password_timeout` ahora cierran stdin explícitamente.
  Con `--nointeraction` bw no debería leer, pero el TUI corre en raw
  mode: un fd heredado podría hacer que bw "robe" keystrokes o se
  cuelgue esperando input.
- **`list_items_timeout_secs` configurable en `config.toml`** — nuevo
  setting (default `60`) reemplaza el `LOCAL_OP_FALLBACK_TIMEOUT` de
  10 s para `bw list items` / `bw list items --trash`. Decrypt + JSON
  serialization en vaults grandes legítimamente puede llevar varios
  segundos; el ceiling defensivo se mantiene para detectar procesos
  realmente colgados.
- **Cache de counts del sidebar de folders/collections** — los badges
  de cada row se calculaban con `app.items.iter().filter().count()`
  por row por frame. En un vault con 5 k items + 20 folders + 30
  collections eso eran ~250 k iteraciones por redraw, y un redraw
  pasa por keystroke. Ahora `App::rebuild_sidebar_counts` arma
  `HashMap<id, count>` una vez por mutación de items y el render lee
  en O(1) por row.
- **Memberships popup: sort una sola vez al abrir** — el render
  ordenaba las collections con `sort_by_key(|c| c.name.to_lowercase())`
  por frame, allocando un `String` lowercased por collection cada
  redraw. Sort ahora corre en `flows::memberships::open`; el
  `MembershipState` queda pre-ordenado.
- **Login parallelization** — los reads secundarios post-auth
  (`bw list folders`, `bw list organizations`, `bw list collections`,
  `bw import --formats`) se disparan ahora en paralelo vía
  `VaultPort::parallel_session_data` (default sequencial; override en
  `BwCliAdapter` que clona el adapter por thread). Como el costo
  dominante es Node cold-start (~500 ms por spawn), la latencia
  post-`bw list items` baja de ~4× a ~1× una sola invocación de bw.

### v1.2 polish sprint

- **`bw move` (mover personal → org)** (#12) — nuevo `Alt+M` desde el
  detail screen de un item personal abre el popup multi-select de
  collections, pre-poblado con la org del usuario. `Enter` ejecuta
  `bw move <id> <org> <ids>` directamente; cancel deja el item
  intacto. Limitación scope: solo funciona si el usuario tiene
  exactamente 1 organisation; multi-org muestra error toast con
  instrucciones para usar shell directamente.
- **Dropdown de import formats** (#15) — el popup de import ahora
  muestra los formatos como dropdown navegable con `← →` en lugar
  de pedirle al usuario que tipee `bitwardenjson` / `lastpasscsv`
  exactamente. La lista se carga al login con `bw import --formats`
  y se cachea hasta el próximo restart. Default sigue siendo
  `bitwardenjson`. Si la enumeración falla (bw error, offline), el
  popup cae a un fallback de un solo elemento (`bitwardenjson`).
- **TOML escape para emails** (#20) — `escape_toml_basic`/`unescape_toml_basic`
  cubren el round-trip de emails con `"` o `\` en el config. La
  validación previa rechaza estos caracteres en el flow normal,
  pero defense-in-depth para configs hand-edited.
- **Timeout defensivo en `bw_run` local-only** (#21) — `bw_run` y
  `bw_run_with_password` ya no son `Command::output()` sin
  límites; ahora wrappean `bw_run_timeout(10s)` como fallback. Si
  bw cuelga por bug en una op local (unlock, list cached, get
  totp, fingerprint), la TUI sale con un toast en vez de
  congelarse.

### Auth

- **Reprompt (master-password reverify) honored** — items marked with
  the Bitwarden `reprompt` flag now gate their secret-exposing
  actions behind a master-password popup. Triggers: `Alt+C` on a
  password (vault list), `Alt+C` on a hidden field in detail (password
  / TOTP / hidden custom field), and F2 reveal in either read-mode or
  edit-mode. Verification runs `bw unlock` and replaces the in-memory
  session key with the freshly-issued one; failures keep the popup
  open with an inline error strip. No caching: every protected action
  re-prompts, matching the official Bitwarden GUI.
- **Item parser: `reprompt: u8`** — round-tripped from `bw list items`
  / `bw get item`. `0` = no extra check, anything else = protected.
  The `Item::needs_reprompt()` helper returns `true` for any non-zero
  value so future schema additions don't silently downgrade.

### Features

- **Filtro por URL en la search bar** (F11) — escribe `url:github` o
  `url:https://example.com` en la búsqueda y bytewarden filtra
  exclusivamente por las URIs de los items (substring,
  case-insensitive), saltando el ranking fuzzy normal. Cubre el use
  case típico de "qué credenciales tengo para este sitio?" sin
  agregar UI nueva — es una capa fina sobre la search existente.
  `url:` solo (sin substring) deja la lista entera, igual que cuando
  no hay query.
- **Help popup actualizado** — el F1 ahora documenta las hotkeys
  agregadas en este sprint: `Alt+L` (assign collections en edit /
  create), `← →` (cycle Organization en create, cycle 2FA method
  en login), reprompt-protected actions, e indicators de lista
  (`★` / `🔒` / `👥`). Sección dedicada por contexto: detail edit-
  mode, create form, login two-factor.
- **Crear items directamente en una organisation** (resto de F6) — el
  create form ahora muestra una fila `Organization` cyclable con
  `← →` cuando el usuario es miembro de al menos una organisation
  (default `Personal`). Cuando se elige una org, se inyecta debajo
  una fila `Collections` (read-only, `Alt+L` abre el mismo
  multi-select que ya usaba edit). El patcher de create incluye
  `organizationId` y `collectionIds` cuando el row Organization
  apunta a una org real. Validación inline: items de org requieren
  ≥1 collection antes de mandar el `bw create item`. Personal-only
  accounts no ven la fila — flujo idéntico al anterior.
- **Indicadores visuales en la lista del vault** (F19/F20) — cada
  fila ahora muestra hasta tres prefijos: `★` (favorite, ya estaba),
  `🔒` (reprompt-protected) y `👥` (item de organisation). Permite
  saber de un vistazo qué items van a pedir master-password al
  copiar y cuáles son compartidos, sin tener que abrir el detail
  o intentar la acción a ciegas.
- **Asignar colecciones desde el edit form** (F6) — items que ya
  pertenecen a una organisation ahora muestran un row read-only
  `Collections` con la lista actual (`Eng, Ops`). Con el cursor en esa
  fila, `Alt+L` abre un popup multi-select con todas las colecciones
  visibles de la org del item. `j/k` navega, `Space` toggle, `Enter`
  aplica, `Esc` cancela. La validación bw "≥1 colección por item de
  org" se enforza inline con error strip antes de que el save salga.
  Los UUIDs no visibles (collection que el usuario no ve por
  permisos) se preservan en el round-trip — no se silencian. Crear
  items nuevos con asignación a org y `bw move` para mover personal
  → org siguen pendientes (sub-tareas separadas).
- **Filtrar por colección desde el sidebar** — el panel `[1]` ahora
  surfacea, además de los folders personales, todas las collections
  visibles (sólo lectura por ahora). Cada collection aparece como
  `Org / Name` con icono `👥`; los folders mantienen su `📁`. Personal-
  only accounts no ven ningún cambio (la sección queda vacía). El
  filtro de colección comparte la misma columna que folders y
  funciona con la misma navegación (`j/k/Enter`). Los counts por
  collection se calculan en el render.
- **Domain `Item` modela `organizationId` y `collectionIds`** — antes
  se ignoraban en el parser. Ahora bytewarden round-trippea ambos
  campos sin perderlos en `bw edit item`. La asignación de
  collections desde la TUI sigue siendo un follow-up (F6).
- **`FolderFilter::Collection(uuid)` + `matches(folder_id, collection_ids)`**
  — el filtro del sidebar es ahora un único enum tagged que cubre
  meta-rows + folder + collection.

### Security

- **Vault-data zeroization on drop** — every domain payload that holds
  vault credentials (`Item`, `LoginData`, `CardData`, `SshKeyData`,
  `IdentityData`, `Field`, `UriData`, `Attachment`) now derives
  `Zeroize` + `ZeroizeOnDrop`. When an item drops — vault unloaded on
  lock, logout, or app shutdown; clones consumed by a flow; trash
  bucket discarded after a restore — every byte of every owned
  `String` is overwritten with zeroes by the compiler-generated
  `Drop`. Closes the window where a heap dump or swap-out could leak
  the contents of the unlocked vault. Search-side cache
  (`LoweredItem`) gets the same treatment.
- **`get_item_json` returns `Zeroizing<String>`** — the raw JSON
  buffer pulled out of `bw get item` carries plaintext credentials.
  The port now wraps it so the buffer is scrubbed when the caller
  drops it. The two intermediate JSON payloads built inside
  `do_save_edit` and `do_toggle_favorite` (the patched / re-serialised
  body that goes into `bw edit item`) are wrapped at the call site
  for the same reason.
- **Edit-form buffers wrapped in `Zeroizing`** — `EditField.value`
  (every row of the create / edit form, hidden or not) and
  `GeneratorState.result` (the freshly-generated password before the
  user copies or uses it) are now zeroized on drop. The wrapper
  `Deref`s to `&String`/`&mut String` so all the cursor logic stays
  identical; the only callsites that needed a touch-up were the
  three or four that did `field.value = some_string` (now
  `Zeroizing::new(some_string)`) and the test asserts that compared
  `value` directly to a `&str`.
- **Compile-time guard** — a structural test asserts every domain
  payload implements `Zeroize`. If a future refactor drops the derive,
  the suite fails to build before any vault data leaks.

### Performance

- **Cached fuzzy search** — every search keystroke used to allocate one
  lowercased copy of the item name, username, every URI and the notes
  *per item* per *keystroke*. Items now carry a parallel
  `Vec<LoweredItem>` populated once at load (and refreshed only when
  the underlying items change), so the hot path is allocation-free.
- **Cached filtered list** — `App::filtered_items` is no longer the
  per-frame O(N) re-filter-and-rerank it used to be. A `Vec<usize>`
  index cache is rebuilt only on mutations (load, sync, create, edit,
  delete, restore, sort, search-query change, filter change,
  folder-filter change). Reads are O(K) — one indirection per visible
  row. Ad-hoc benchmark with 5 000 items and an active query lands at
  **~250 µs per frame**, well below the 80 ms frame budget.
- **`sort_items` uses `sort_by_cached_key`** — `to_lowercase` is now
  called once per item per sort instead of once per comparison.

### Auth

- **Two-factor login: Authenticator + Email + YubiKey** — bytewarden
  now distinguishes the `two-step login` prompt from the `new device`
  prompt and drives `bw login --method N` for all three permanent 2FA
  methods bw documents (`0` Authenticator, `1` Email, `3` YubiKey).
  When the popup appears, `← →` cycles the method on the code field;
  the chip under the input shows the active selection and label hints
  describe the expected source ("TOTP from your authenticator app",
  "sent to your email", "touch your YubiKey"). Default is
  Authenticator — the most common case. Accounts with a permanent
  second factor enrolled can finally log in directly from bytewarden
  instead of having to bounce through the official client.
- **Login outcome refactor** — the domain `LoginOutcome::NeedsOtp`
  variant has been split into `NeedsDeviceVerification` and
  `NeedsTwoFactor` so the auth flow can route to the correct port
  method without ambiguity. The adapter's prompt classifier consults
  the 2FA patterns first to keep mixed prompts (e.g. *"Two-step Login.
  Enter the verification code:"*) on the right path.

### Security

- **Clipboard auto-clear** — every clipboard write that carries a secret
  (passwords, usernames, TOTP codes, copied detail-view fields, generated
  values, Send URLs) is now wiped automatically after a configurable
  delay. Default `30` seconds, matching the Bitwarden GUI; set
  `clipboard_clear_secs = 0` in `~/.config/bytewarden/config.toml` to
  disable. The clear is contingent on the clipboard still holding the
  value bytewarden wrote — copying something else cancels the wipe so
  your new selection survives.
- **2FA codes go via stdin, not argv** — the new
  `login_with_two_factor` path reuses the OTP plumbing: `--method 0` in
  the argument list, six-digit code piped over the child's stdin, so
  the secret is invisible to `ps` exactly like the password and the
  device-verification OTP already were.

## [1.0.0] — 2026-05-03

First public release.

### Item management
- All five Bitwarden item types: Login, Secure Note, Card, Identity, SSH Key.
- Inline edit mode with multi-URI logins, custom fields (text / hidden / boolean),
  type cycling and field rename popup.
- Custom fields of type `linked` from the official Bitwarden GUI are preserved
  read-only across edits — the patcher keeps `linkedId` intact instead of
  silently dropping it.
- Trash / restore / permanent delete with confirmation popups.

### Folders, send, attachments, memberships
- Full folder CRUD with inline create / rename / delete popups.
- Bitwarden Send (text) creation with auto-copy of the URL on success.
- Attachment upload, **download** (path picker pre-filled to `~/Downloads/`) and
  **delete** (confirmation popup).
- Read-only memberships view (organisations + collections).

### Auth & session
- Three login methods — master password, headless API-key, SSO browser flow.
- "New device" OTP detection with **stdin-fed** verification code (no `--code`
  in `argv`, never visible in `ps`).
- Session persistence behind a `Keep session` checkbox: per-PPID file at
  `${XDG_RUNTIME_DIR}/bytewarden/session-<ppid>` (mode `0600`), wiped when the
  parent shell dies or after 24 h regardless of liveness.

### Security hardening
- Wall-clock timeouts on every `bw` call that touches the network — no more
  30-second TUI freezes when the server is slow.
- Master password fed via `BW_PASS_INPUT` env var, OTP via stdin — neither
  value reaches `argv`.
- Session key, password buffer and OTP buffer wrapped in `zeroize::Zeroizing`
  — overwritten with zeroes on drop.
- `~/.config/bytewarden/config.toml` created atomically with `0600` mode via
  `OpenOptions::mode`; the directory is `0700`.
- `cmd_log` is wiped on logout.
- Pre-flight input validation on email, server URL, export/import paths,
  folder names and custom-field labels — clear toasts before paying for a
  network round-trip.
- Zero `unsafe` blocks in production code.

### TUI
- Hexagonal architecture: domain / ports / adapters / TUI driving adapter.
- Themable via `[theme]` block in `config.toml` (18 colour keys, all optional).
- Per-screen scoped help popup (F1) with vertical and horizontal scrolling.
- Mouse capture: click panels, click items, scroll wheel everywhere,
  Shift+wheel for horizontal pan in the help popup.
- Auto-lock after configurable inactivity.
- Fuzzy search across name / username / URI with weighted scoring.
- Optional persistent debug log via `BYTEWARDEN_DEBUG=1` →
  `~/.bytewarden.log`.
- Graceful "terminal too small" fallback when the area is below 60×18.

### Quality
- 190 unit tests + 4 doctests passing.
- ~95 % line coverage on the testable layer (domain, port-pure adapters, TUI
  helpers, validation, session-file).
- `cargo clippy -D warnings` and `cargo fmt --check` clean on Rust 1.95.
- Rust toolchain pinned to 1.95.0 via `rust-toolchain.toml` — local
  development and CI share the same compiler / clippy / rustfmt versions,
  so a stable bump cannot silently break the gate. Bumping the channel
  in the file is the single source of truth (honoured by `rustup`
  locally and by the CI install step), and the cargo cache key hashes
  the file so a toolchain bump auto-invalidates the cache.
- GitHub Actions CI on every push and pull request against `main` and
  `dev`.

[1.0.0]: https://github.com/vibecoded-software-factory/bytewarden/releases/tag/v1.0.0
