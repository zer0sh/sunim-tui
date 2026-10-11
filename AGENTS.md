# AGENTS.md

Guide for coding agents and contributors working on **sunim**, a Rust TUI for
managing Pantheon sites through the Terminus CLI. Read this before changing
code. The user-facing documentation is `README.md`.

## Quick reference

| Task | Command |
|------|---------|
| List targets | `make` |
| Build / run | `make build` / `make run` |
| Tests | `make test` |
| Lint (warnings are errors) | `make lint` |
| Format | `make fmt` |
| **Before finishing any change** | `make check` (fmt-check + clippy `-D warnings` + tests) — must pass |
| Size-optimized / static build | `make release` / `make static` |
| Create a release tag | `make tag`, then `git push origin vX.Y.Z` |

CI (`.github/workflows/ci.yml`) runs `make check` on Linux and macOS.

## Toolchain and constraints

- Rust **1.98+**, **edition 2024** (`rust-version` is set in `Cargo.toml`). Cargo
  must be available in `$PATH` or `~/.cargo/bin`. Let chains
  (`if let … && let …`) are fine and preferred over nested `if`s.
- **Unix only**: Linux and macOS (`storage.rs` uses `std::os::unix` permissions;
  WSL on Windows is supported). Don't add Windows-specific code paths without
  discussing it first.
- Dependencies are deliberately few (ratatui, crossterm, tokio, serde,
  serde_json, anyhow, dirs, toml). Don't add a crate for something a few lines
  can do (e.g. date formatting lives in `model::format_timestamp`).
- Code, comments, UI text and commit messages are in **English**.

## Architecture

```
main.rs ── event loop ──► ui::draw(&App)          (pure rendering, no side effects)
   │                          ▲
   ├─ key events ──► App::handle_key ──► App::spawn(label, task) ──► tokio task
   │                                                                     │
   └─ mpsc channel ◄──────── TaskDone { id, AppEvent } (one per task) ───┘
                 └─► App::handle_task_done ──► handle_app_event (+ cache update)
```

| File | Responsibility |
|------|----------------|
| `src/main.rs` | CLI flags (`--version`, `--help`), TTY check, loads config, creates `App`, `ratatui::try_init()`, panic hook, event loop (drain events → draw → poll keys → tick). |
| `src/app/mod.rs` | State, `spawn`/`TaskDone`, results handling, cache usage, loading data. |
| `src/app/keys.rs` | Keyboard: overlays, dialogs (new/delete multidev), per-screen keys, selector, command bar. |
| `src/app/tests.rs` | Behaviour tests driving `handle_key` / `execute_command` / `handle_app_event`. |
| `src/model.rs` | Data types deserialized from Terminus (`Site`, `Environment`, `Backup`) and pure helpers: sorting, validation, filtering, timestamp formatting. |
| `src/terminus.rs` | The **only** place that runs `terminus`. One async fn per command. |
| `src/storage.rs` | `Config` (token + selected sites) and `Cache` (last results) load/save, via one private atomic writer. |
| `src/system.rs` | OS integration (opening URLs). |
| `src/ui/keymap.rs` | **Single source of truth** for key bindings and commands (`static` sections). Footer hints and the help overlay are generated from it. |
| `src/ui/*.rs` | One file per screen/overlay. `ui/mod.rs`: `chrome` (header + footer, returns the body), `with_footer`, the top-right activity indicator, `highlight_style`, `filter_label`, `centered`. `dialogs.rs`: multidev dialogs. `help.rs`: scrollable help. |

### Rules of the async model

- **Never block the UI thread.** Anything that runs a process or does I/O
  that can hang goes through `App::spawn(log_line, async { … AppEvent })`.
- `App::spawn` registers the task in `running` (id → short label derived from
  the log line, shown top right) and `handle_task_done` removes it. A task
  sends exactly one `TaskDone` — `spawn` already converts a panicking task
  into an `AppEvent::Op` error, so don't bypass it with a bare `tokio::spawn`.
- Loading state is `App::is_loading()` / `App::activity()` (derived from
  `running`). Don't reintroduce a boolean `loading` flag or a counter.
- **In `cfg(test)`, `spawn` never runs the task**: it only records it in
  `running`. Tests assert on those labels (`started(&app)`), so it is safe to
  test code paths that would call terminus.
- Log lines passed to `spawn` start with `terminus <command> <target>` and put
  flags last: the indicator label is cut at the first `--flag`.
- Results tied to a context carry it (`AppEvent::Envs { site, .. }`,
  `AppEvent::Backups { site, env, .. }`). They always update the cache but
  are **not shown if the user has navigated away**. New context-dependent
  results must do the same.
- Mutating operations use `AppEvent::Op { what, result, refresh }`; `what`
  names the operation in error messages ("Deploy failed: …") and `refresh`
  says what to reload on success.
- Report user-visible problems with `set_error` (stays until `Esc`) and
  successes with `set_status` (expires after `STATUS_TTL`). Both also go to the
  log panel. Use `push_log` directly only for informational lines.

## Cache

- `storage::Cache` (`~/.cache/sunim-tui/cache.json`) holds the last sites,
  environments per site and backups per `site.env`, each with `fetched_at`,
  plus the account (`user`) it belongs to. It is disposable: unreadable or
  other-`CACHE_VERSION` files load as empty. Bump `CACHE_VERSION` when the
  layout changes incompatibly.
- Screens show cached data immediately and refresh in the background; the
  indicator shows the data age (`App::data_updated`). Every successful list
  result updates the cache and saves it (errors are only logged).
- `App::start` is optimistic: with a cached user + sites it opens the
  dashboard right away; a failed session check sends the user to login.
  `adopt_user` discards the cache when another account signs in, sites
  refreshes prune entries of deleted sites, and logout deletes the file.
- Keep the cursor on the same item after refreshes (`reselect` by key), not
  just the same index.

## Terminus integration

All calls go through `terminus::run(args, timeout)`, which:

- appends `--no-interaction --no-ansi`, closes stdin and sets
  `kill_on_drop(true)`;
- enforces a timeout (`QUERY_TIMEOUT` = 2 min for reads/login,
  `WORKFLOW_TIMEOUT` = 30 min for anything that waits on a Pantheon workflow);
- on failure returns the cleaned stderr/stdout (`clean_message` strips
  `[error]`/`[notice]` tags).

Facts about Terminus 4.x output that the code relies on (verified against
Terminus 4.3.3):

- `site:list`, `env:list`, `backup:list` with `--format=json` return an
  **object keyed by id** (site UUID, env id, backup id), not an array. Key
  order is meaningless — always sort (`model::sort_*`). `parse_list` accepts
  both shapes and fails loudly on a malformed entry (never drop rows silently).
- `env:list` fields: `id, created, domain, connection_mode ("git"|"sftp"),
  locked, initialized`. There is **no** `on_server_development` field.
- `site:list` fields: `name, id, plan_name, framework, region, owner, created,
  memberships, frozen`.
- `backup:list` fields: `file, size ("20.4MB"), date, expiry (Unix epoch
  floats), initiator`. The element (code/database/files) is the last `_`
  segment of the file name.
- Most commands print their success notices on **stderr** and nothing on
  stdout; use `Output::message(fallback)`.
- `auth:whoami` may exit 0 with empty stdout when logged out — treat empty as
  "not logged in".
- Terminus keeps its own session in `~/.terminus`; commands other than
  `auth:login` don't take a token.

Use the lenient deserializers in `model.rs` (`lenient_string`, `lenient_bool`,
`lenient_f64`) for any new field: Terminus is not strict about JSON types.

### Adding a Terminus operation (checklist)

1. Check the real command: `terminus <cmd> --help`. For read-only commands you
   may inspect real output (`--format=json`); **never run mutating commands
   against the user's Pantheon account while developing or testing.**
2. Add an async fn in `terminus.rs` using `run(…, QUERY_TIMEOUT |
   WORKFLOW_TIMEOUT)`. Pass `--yes` only when the UI has already asked the
   user for confirmation.
3. Trigger it from `app/keys.rs` via `self.spawn("terminus <cmd> <target>", …)`
   returning `AppEvent::Op` (or a dedicated event if it returns data).
4. Destructive or production-affecting actions (delete, deploy, anything on
   live, switching to Git mode) go through `Overlay::Confirm` with a new
   `ConfirmAction` variant. Only `y`/`Y` confirms.
5. Validate user input before spawning (see `model::validate_multidev_name`).
6. Add tests, then add the key/command to `ui/keymap.rs` and update
   `README.md` (see "Keep docs in sync").

## UI and key handling conventions

- `handle_key` order matters: overlays (help, command bar, confirm, the two
  multidev dialogs — each captures every key) → active `/` filter (captures
  every key while editing, so `h`, `l`, `:` can be typed) → global keys (`h`/`?`,
  `:`, `l`, `PgUp`/`PgDn`, `Esc` dismissing a message) → per-screen handler.
- Dialog state lives in the `Overlay` variant itself (`MultidevForm`,
  `DeleteForm`); validation is a method on the form (`problem`, `confirmed`)
  so the UI and the key handler agree. Destructive actions need either a
  `y/N` confirm or, for irreversible deletes, typing the name.
- `Esc` first dismisses a message, then clears an active filter, then goes
  back. `q` goes back (quits on the dashboard). `Ctrl-c` always quits
  (handled in `main.rs`).
- Lists use `navigate()` for `j/k`, arrows, `g/G`, `Home/End`. Keep new list
  screens consistent with it and with `highlight_style()`.
- Text inputs must ignore Ctrl/Alt chords (`is_text_input`).
- Letters already taken on the environments screen: `c d D m w o b n r q / y`.
  Global: `h ? l :`. Pick new keys that don't clash.
- Every screen draws through `ui::chrome` (or `with_footer`), so the message
  line and the wrapped footer hints are consistent. Don't hand-write hint
  strings: add the key to `ui/keymap.rs` with a short `hint`.
- `ui/` code only reads `&App` (the one exception is `help_max_scroll`, a
  `Cell` the help overlay writes so scrolling stops at the last line); state
  changes belong in `app/`.

## Site selection

- `Config::selected_sites` is `None` until the first-run selector is completed;
  while `None`, the selector opens automatically after sites load and can't be
  cancelled (only saved with ≥ 1 site).
- `App::visible_sites()` = selection applied; `listed_sites()` = selection +
  `/` filter (sites screen); `selector_sites()` = all sites + selector filter.
  Dashboard and `:site` partial matching use the visible set; `:site` with an
  exact name works for any site.
- Saving drops names that no longer exist on Pantheon.

## Configuration file

`~/.config/sunim-tui/config.toml` (`dirs::config_dir()`, honours
`$XDG_CONFIG_HOME`). `storage::save` writes a `0600` temp file and renames it
into a `0700` directory — keep it atomic and private. New fields must use
`#[serde(default)]` so older files keep loading, and the token must only be
written after Terminus accepted it.

## Testing

- Unit tests live next to the code (`#[cfg(test)] mod tests`). Pure logic
  (model, parsing, storage, key handling, commands) is covered; add tests for
  new behaviour.
- Tests must **not** spawn Terminus. `App` tests drive `handle_key`,
  `execute_command` and `handle_app_event` with fake events; paths that call
  `spawn` need a tokio runtime and would run real commands, so avoid them.
- Storage tests use `storage::tests::temp_config_path()` / `temp_path(name)`.
- Manual end-to-end check: run the binary with an isolated config and cache
  so the user's real files are untouched, and stick to read-only actions:
  `XDG_CONFIG_HOME=$(mktemp -d) XDG_CACHE_HOME=$(mktemp -d) cargo run`.

## Keep docs in sync

A change to keys, commands or behaviour must update, in the same change:

1. `src/ui/keymap.rs` — the binding/command with its footer `hint` and full
   `help` text. Footer and help overlay are generated from it, and tests check
   that every screen has a section and every command is documented.
2. `README.md` (Features, Key Bindings, Command Bar, Project Structure).
3. This file, if it changes architecture or conventions.

## Releases

- `.github/workflows/release.yml` runs on tags `vX.Y.Z[-pre]`: checks the tag
  equals `version` in `Cargo.toml`, runs `make check`, builds
  `x86_64/aarch64-unknown-linux-musl` (static, via `cargo zigbuild`) and
  `x86_64/aarch64-apple-darwin`, smoke-tests `sunim --version` where the
  runner can execute it, and publishes `sunim-<tag>-<target>.tar.gz` (binary +
  README + LICENSE), per-archive `.sha256` files and `SHA256SUMS`.
- `sunim --version` must keep printing exactly `sunim <version>`; the release
  smoke test compares it with the tag.
- The `[profile.release]` in `Cargo.toml` is tuned for size. **Keep
  `panic = "unwind"`** (the default): `App::spawn` relies on catching
  panicking tasks, and `abort` would kill the app without restoring the
  terminal.
- Don't add dependencies that need C libraries or OpenSSL: they would break
  the static musl builds.
- Validate workflow edits with `actionlint` before pushing.

## Safety

- Never commit or print machine tokens. The login log line masks the token
  (`--machine-token=***`); keep it that way.
- Don't run `env:deploy`, `multidev:*`, `connection:set`, `backup:create`,
  `env:clear-cache` or `auth:logout` against the real account during
  development unless the user explicitly asks.
