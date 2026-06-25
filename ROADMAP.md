# Roadmap

`bytewarden` is a Bitwarden-CLI TUI. v1.x covers the full chain `login →
vault → detail` with complete CRUD over items, folders, attachments, Sends,
import/export, plus the generator, memberships and HIBP check. Everything
below is on the table for future versions. Items are roughly ordered by how
soon they're worth doing, not committed.

## Soon — polish on the existing surface

- [ ] **Assign collections on create** — today only existing org items can be
  re-assigned (`Alt+L` in edit mode). Let the create form pick an org +
  collections up front (`bw create` then `move`).
- [ ] **Richer Sends** — file Sends, password-protected Sends, max-access
  count, hidden text. Currently text-only with a 1–31 day expiry.
- [ ] **More 2FA methods** — surface every non-browser factor cleanly
  (Authenticator / Email / YubiKey are in). The browser-based factors (Duo,
  WebAuthn, U2F) stay out of scope.
- [ ] **Search history / saved filters** — recall recent `search_query` +
  filter combos so common lookups don't get re-typed.

## Then — broader features

- [ ] **Org-aware item management** — create directly into an organisation,
  move between collections, manage sharing from the detail screen.
- [ ] **Bulk actions** — multi-select in the vault list for bulk move /
  delete / favorite.
- [ ] **Attachment preview** — inline preview for text attachments (mirrors
  jewel's S3 object preview pattern).

## Cross-cutting — architecture

- [ ] **Worker thread + `mpsc` (de-freeze the UI).** Today bytewarden is
  synchronous: each `bw` call blocks the render loop for its whole duration
  (see `CLAUDE.md` → "Execution model"). The sibling project **jewel** runs a
  worker thread that owns the port and serves requests over `mpsc` so the
  render thread never blocks, plus a background lane for long non-interactive
  work. Porting that model here (the spinner would actually animate during a
  slow `bw sync`, and `Ctrl+C` would stay instant) is the **big** improvement
  — a deliberate, explicit decision, not a side effect of another change.
  Keep the per-call timeouts either way.
- [ ] **Snapshot tests for the view layer** — none of the sibling projects
  have these yet; would unlock confident refactors of `view/*` and `input/*`
  (the layers currently below the ~95 % coverage of `domain`/`adapters`).
- [ ] **Bw process reuse / warm Node** — the `bw` CLI pays a Node cold-start
  per spawn; investigate `bw serve` or batching to cut latency (would compose
  well with the worker-thread change above).

## Non-goals

- Replacing the `bw` CLI with a reimplementation of the Bitwarden protocol /
  an SDK. Wrapping `bw` is the feature, not a workaround.
- Writing secret values to disk beyond the user-chosen export path. Backups
  are Bitwarden's own; bytewarden keeps secrets in `zeroize`d memory only.
- Caching the master password to skip reprompt. Reprompt re-verifies every
  time on purpose (matches the official Bitwarden GUI).
