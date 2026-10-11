# sunim

A keyboard-driven terminal UI for managing [Pantheon](https://pantheon.io) sites via the [Terminus CLI](https://terminus.pantheon.io).

## Features

- **Seamless sign-in** — reuses an existing Terminus session. Otherwise enter your Machine Token once; it is saved only after Terminus accepts it and is used to log in again automatically when the session expires.
- **Site selector** — on first launch you pick which sites you want to manage; only those appear on the dashboard and in the sites list. Change the selection any time with `S` or `:select`.
- **Dashboard** — logged-in user, site counts by framework (Sites / WordPress / Drupal / Other) and a preview of the first 5 sites (A→Z) with framework and plan.
- **Sites list** — sorted by name, filterable with `/`, frozen sites are tagged.
- **Environments** — dev, test, live first, then multidevs, with Git/SFTP mode, locked and uninitialized tags. From there:
  - filter environments by name with `/`;
  - copy environment URL to clipboard with `y`;
  - clear cache, deploy (dev→test, test→live), wake, open in the browser;
  - toggle Git/SFTP mode (dev and multidevs);
  - create a multidev in a dialog: type the name (validated as you type) and pick whether its database and files come from dev, test or live;
  - delete a multidev in a dialog that requires typing its name, with the option to keep its git branch (dev, test and live can't be deleted);
  - browse and create backups (copy archive filename with `y`).
- **Instant start with cache** — the last sites, environments and backups are saved in `~/.cache/sunim-tui/cache.json` and shown immediately (also on the next launch) while fresh data loads in the background.
- **Activity indicator** — the top-right corner shows what is loading (`⠋ env:list my-site (+1)`) or how old the data on screen is (`updated 5m ago · r refresh`). `r` refreshes any screen.
- **Confirmation guard** — deploying, switching to Git mode (which discards uncommitted SFTP changes) and logging out ask `y/N`; deleting a multidev requires typing its name.
- **Vim-style command bar** — press `:` to run commands, e.g. `:site my-cool-site` (partial names work).
- **Output log panel** — press `l` to see every Terminus command that ran and its result; scroll with `PgUp`/`PgDn`.
- **Readable footer** — a message line plus every shortcut of the current screen, wrapped over as many lines as the terminal needs.
- **Complete help** — `h` or `?` opens a scrollable reference with everything: the current screen first, then every other screen, dialog, command, tag, indicator and file.
- **Never freezes** — every Terminus call runs in the background with a timeout, stdin closed and `--no-interaction`. Results that arrive after you navigated elsewhere are cached but not shown.
- **Safe terminal handling** — the terminal is restored even if the app crashes.

## Requirements

- **Linux or macOS** — token storage uses Unix file permissions (`0600`/`0700`); Windows native is not supported, but WSL on Windows is supported.
- **Rust 1.98+ and Cargo** (explicit requirement to build from source) — `sunim-tui` uses edition 2024 and sets `rust-version = "1.98"`. Cargo must be available in `$PATH` or `~/.cargo/bin`. Install or update with:
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  rustup update stable
  ```
- **[Terminus CLI](https://docs.pantheon.io/terminus/install)** — installed and available in `$PATH` (on macOS, sunim also checks standard Homebrew locations `/opt/homebrew/bin/terminus` and `/usr/local/bin/terminus`).
- **A Pantheon [Machine Token](https://docs.pantheon.io/machine-tokens)** (not needed if you are already logged in with `terminus auth:login`).
- **Browser opener** — `open` (macOS), `xdg-open` (Linux), or `wslview` (WSL).
- **Clipboard** — OSC 52 sequence (works out of the box across modern terminals and SSH) plus `pbcopy` on macOS and `wl-copy`/`xclip` on Linux.

## Install

### From a release (prebuilt binary)

Each [GitHub release](../../releases) ships a small, self-contained binary (~1.5 MB; Linux builds are fully static):

| Platform | Archive |
|----------|---------|
| Linux x86_64 | `sunim-vX.Y.Z-x86_64-unknown-linux-musl.tar.gz` |
| Linux ARM64 | `sunim-vX.Y.Z-aarch64-unknown-linux-musl.tar.gz` |
| macOS Intel | `sunim-vX.Y.Z-x86_64-apple-darwin.tar.gz` |
| macOS Apple Silicon | `sunim-vX.Y.Z-aarch64-apple-darwin.tar.gz` |

```bash
tar -xzf sunim-vX.Y.Z-<target>.tar.gz
sha256sum -c sunim-vX.Y.Z-<target>.tar.gz.sha256   # optional, macOS: shasum -a 256 -c
install -m 755 sunim-vX.Y.Z-<target>/sunim ~/.local/bin/sunim
sunim --version
```

On macOS the binary is not signed; if Gatekeeper blocks it, run `xattr -d com.apple.quarantine ~/.local/bin/sunim`.

### From source

```bash
# Run inside the project directory
make install          # same as: cargo install --path . --locked

# The binary is installed as `sunim`
sunim
```

## Development

```bash
make            # list all targets
make run        # build and launch
make check      # what CI runs: cargo fmt --check, clippy -D warnings, tests
make release    # size-optimized binary in target/release/sunim
make static     # fully static Linux binary (musl)
```

CI (GitHub Actions, `.github/workflows/ci.yml`) runs `make check` on Linux and macOS. See [AGENTS.md](AGENTS.md) for architecture and contribution conventions.

### Releasing

Releases are built and published by `.github/workflows/release.yml` when a version tag is pushed:

1. Bump `version` in `Cargo.toml` (and run `cargo build` so `Cargo.lock` follows), commit.
2. `make tag` — runs `make check`, requires a clean working tree and creates the annotated tag `vX.Y.Z` from `Cargo.toml`.
3. `git push origin vX.Y.Z`

The workflow fails if the tag doesn't match `Cargo.toml`, runs `make check`, builds the four targets (Linux via `cargo zigbuild`, macOS natively), smoke-tests `sunim --version`, and publishes the archives plus `SHA256SUMS` with auto-generated release notes. Tags with a hyphen (e.g. `v1.2.0-rc.1`) become pre-releases.

Release binaries use the size profile in `Cargo.toml` (`opt-level = "z"`, LTO, one codegen unit, stripped).

## Usage

```
sunim             # launch the TUI
sunim --version   # print version
sunim --help      # short usage
```

1. **Sign in.** If `terminus auth:whoami` already reports a user, sunim goes straight to the dashboard. If not, and a token is stored, it runs `terminus auth:login` with it. Otherwise the login screen asks for a Machine Token; paste it and press `Enter`.
2. **Choose your sites.** The first time, the site selector opens: `Space` toggles a site, `a`/`n` select all/none, `/` filters, `Enter` saves. You must pick at least one site. You can change it later with `S` (dashboard or sites list) or `:select`.
3. **Work.** Open a site with `Enter` to manage its environments.

### Configuration file

Settings live in `~/.config/sunim-tui/config.toml` (the platform config dir; `$XDG_CONFIG_HOME` is honoured). The file is written atomically with mode `0600` inside a `0700` directory:

```toml
token = "…"                                    # saved only after a successful login
selected_sites = ["my-site", "another-site"]   # sites shown in the app
```

The token is stored in plain text, protected only by file permissions.

### Cache

The last results of `site:list`, `env:list` and `backup:list` are kept in `~/.cache/sunim-tui/cache.json` (`$XDG_CACHE_HOME` is honoured; same `0600`/`0700` protection). On launch, if the cache belongs to a known account, the dashboard appears immediately from it while the session is checked and the data refreshed; if the session turns out to be expired you are sent to the login screen. The cache is discarded when another account signs in, pruned when sites disappear, ignored if unreadable, and deleted by `:logout`. It is safe to delete at any time.

`:logout` runs `terminus auth:logout`, removes the stored token and deletes the cache; the site selection is kept.

## Key Bindings

Everything below is also in the in-app help (`h` or `?`), and the footer of each screen lists its shortcuts.

### Global (all screens except login)

| Key | Action |
|-----|--------|
| `h` / `?` | Open the help (scroll with `j`/`k`, `PgUp`/`PgDn`, `g`/`G`; close with `Esc`, `q`, `h`, `?` or `Enter`) |
| `:` | Open command bar |
| `l` | Toggle output log panel |
| `PgUp` / `PgDn` | Scroll the log panel (while it is open) |
| `Esc` | Dismiss the current error/status message; if there is none, go back |
| `Ctrl-c` | Quit immediately (works on every screen) |

Success messages disappear after 5 seconds; errors stay until dismissed. List screens also accept `j`/`k`, `↑`/`↓`, `g`/`Home` (top) and `G`/`End` (bottom).

### Login

| Key | Action |
|-----|--------|
| `Enter` | Sign in with the typed/pasted token |
| `Backspace` | Delete last character |
| `Esc` | Clear the input |

### Dashboard

| Key | Action |
|-----|--------|
| `s` / `Enter` | Open sites list |
| `S` | Choose which sites are shown |
| `r` | Refresh site data |
| `q` | Quit |

### Sites

| Key | Action |
|-----|--------|
| `Enter` | Open the selected site's environments |
| `/` | Filter by name (while typing: `↑`/`↓` move, `Enter` keeps the filter, `Esc` clears it) |
| `S` | Choose which sites are shown |
| `r` | Refresh list |
| `q` / `Esc` | Back to dashboard (`Esc` first clears an active filter) |

### Site selector

| Key | Action |
|-----|--------|
| `Space` | Toggle the highlighted site |
| `a` / `n` | Select all / none (only the rows matching the filter) |
| `/` | Filter by name |
| `Enter` | Save the selection (at least one site) |
| `r` | Reload the site list from Pantheon |
| `q` / `Esc` | Cancel without saving (not available on first run) |

### Environments

| Key | Action |
|-----|--------|
| `n` | New multidev — dialog: type the name, `Tab`/`←`/`→` choose the source of database & files (dev, test or live), `Enter` creates, `Esc` cancels |
| `d` | Delete the highlighted multidev — dialog: type its name to unlock `Enter`, `Tab` toggles deleting its git branch too (on by default), `Esc` cancels |
| `c` | Clear cache |
| `D` | Deploy code: dev→test on **test**, test→live on **live** (asks `y/N`) |
| `b` | Open the environment's backups |
| `o` | Open the environment URL in the browser |
| `y` | Copy the environment URL to the clipboard |
| `/` | Filter by name (while typing: `↑`/`↓` move, `Enter` keeps the filter, `Esc` clears it) |
| `m` | Toggle Git/SFTP connection mode on dev or a multidev (switching to Git asks `y/N`) |
| `w` | Wake the environment |
| `r` | Refresh list |
| `q` / `Esc` | Back to sites (`Esc` first clears an active filter) |

Only initialized dev/test/live environments are offered as sources for a new multidev. Names are checked while you type: at most 11 characters, lowercase letters, numbers and hyphens, not reserved, not already used on the site.

### Backups

| Key | Action |
|-----|--------|
| `c` | Create a backup (code, database and files) |
| `y` | Copy the highlighted backup filename to the clipboard |
| `r` | Refresh list |
| `q` / `Esc` | Back to environments |

In every `y/N` confirmation, press `y` to confirm; any other key cancels.

### Command Bar (`:`)

| Command | Description |
|---------|-------------|
| `:q` / `:quit` | Quit |
| `:dashboard` / `:d` | Go to dashboard |
| `:sites` / `:s` | Go to sites list |
| `:select` | Open the site selector |
| `:refresh` / `:r` | Reload the current screen |
| `:site <name>` | Jump to a site. Exact names always work; otherwise a unique partial match among your selected sites opens it, and several matches open the sites list filtered |
| `:multidev:create [name]` | Open the new-multidev dialog, optionally with the name filled in (environments/backups screens) |
| `:multidev:delete <name>` | Open the delete dialog for that multidev (environments/backups screens) |
| `:logout` | Log out of Terminus, forget the stored token and delete the cache (asks `y/N`) |

`Enter` runs the command, `Esc` cancels.

## Project Structure

```
.
├── Makefile              — build / test / lint shortcuts (`make help`)
├── AGENTS.md             — guide for contributors and coding agents
├── .github/workflows/
│   ├── ci.yml            — fmt, clippy, tests on Linux and macOS
│   └── release.yml       — builds and publishes binaries on version tags
└── src/
    ├── main.rs           — CLI flags, terminal setup, panic hook and event loop
    ├── app/
    │   ├── mod.rs        — application state, background tasks, results handling, cache
    │   ├── keys.rs       — keyboard handling, dialogs and command bar
    │   └── tests.rs      — behaviour tests (keys, commands, cache, dialogs)
    ├── model.rs          — Terminus data types, sorting, validation, formatting
    ├── terminus.rs       — async wrappers around the `terminus` CLI (timeouts, parsing)
    ├── storage.rs        — config and cache files (private, atomic writes)
    ├── system.rs         — opening URLs in the browser
    └── ui/
        ├── mod.rs        — draw dispatcher, screen frame (header + footer), activity indicator, confirm
        ├── keymap.rs     — every key binding and command; source of footer hints and help
        ├── dialogs.rs    — new / delete multidev dialogs
        ├── login.rs      — login screen
        ├── dashboard.rs  — dashboard screen
        ├── sites.rs      — site list screen
        ├── selector.rs   — site selector screen
        ├── environments.rs — environment list screen
        ├── backups.rs    — backups screen
        ├── log_panel.rs  — output log panel
        ├── help.rs       — scrollable help overlay (generated from keymap.rs)
        └── command_bar.rs — command bar overlay
```

## License

MIT
