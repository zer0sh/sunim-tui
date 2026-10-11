//! Every key binding and command, in one place. The footer hints and the help
//! overlay are both generated from these tables, so they can't drift apart.
//! The behaviour itself lives in `app/keys.rs` — keep both in sync.

use crate::app::{App, Overlay, Screen};

pub struct Key {
    /// How the key is written, e.g. "j / ↓".
    pub keys: &'static str,
    /// Short label for the footer; empty = help only.
    pub hint: &'static str,
    /// Full description for the help overlay.
    pub help: &'static str,
}

const fn key(keys: &'static str, hint: &'static str, help: &'static str) -> Key {
    Key { keys, hint, help }
}

pub struct Section {
    pub title: &'static str,
    pub keys: &'static [Key],
}

// ── screens ───────────────────────────────────────────────────────────────────

pub static LOGIN: Section = Section {
    title: "Login screen",
    keys: &[
        key(
            "Enter",
            "sign in",
            "Sign in with the typed/pasted Machine Token",
        ),
        key("Backspace", "", "Delete the last character"),
        key("Esc", "clear", "Clear the input"),
        key("Ctrl-c", "quit", "Quit"),
    ],
};

pub static DASHBOARD: Section = Section {
    title: "Dashboard",
    keys: &[
        key("s / Enter", "sites", "Open the sites list"),
        key(
            "S",
            "choose sites",
            "Choose which sites are shown (site selector)",
        ),
        key(
            "r",
            "refresh",
            "Reload sites from Pantheon (cached data stays visible meanwhile)",
        ),
        key("q", "quit", "Quit"),
    ],
};

pub static SITES: Section = Section {
    title: "Sites list",
    keys: &[
        key("Enter", "open", "Open the highlighted site's environments"),
        key("/", "filter", "Filter sites by name"),
        key(
            "S",
            "choose sites",
            "Choose which sites are shown (site selector)",
        ),
        key("r", "refresh", "Reload sites from Pantheon"),
        key(
            "Esc",
            "",
            "Clear an active filter; otherwise back to the dashboard",
        ),
        key("q", "back", "Back to the dashboard"),
    ],
};

pub static SELECTOR: Section = Section {
    title: "Site selector",
    keys: &[
        key("Space", "toggle", "Select / unselect the highlighted site"),
        key("a", "all", "Select every site matching the filter"),
        key("n", "none", "Unselect every site matching the filter"),
        key("/", "filter", "Filter sites by name"),
        key("Enter", "save", "Save the selection (at least one site)"),
        key("r", "reload", "Reload the site list from Pantheon"),
        key(
            "q / Esc",
            "cancel",
            "Leave without saving (not available during first-run setup)",
        ),
    ],
};

pub static ENVIRONMENTS: Section = Section {
    title: "Environments",
    keys: &[
        key(
            "n",
            "new multidev",
            "Create a multidev: type its name, pick where database & files come from (dev/test/live)",
        ),
        key(
            "d",
            "delete",
            "Delete the highlighted multidev (type its name to confirm; optionally keep its branch)",
        ),
        key(
            "c",
            "clear cache",
            "Clear the highlighted environment's cache",
        ),
        key(
            "D",
            "deploy",
            "Deploy code: dev→test on test, test→live on live (asks y/N)",
        ),
        key("b", "backups", "Show the highlighted environment's backups"),
        key("o", "open", "Open the environment's URL in the browser"),
        key(
            "y",
            "copy url",
            "Copy the environment's URL to the clipboard",
        ),
        key("/", "filter", "Filter environments by name"),
        key(
            "m",
            "git/sftp",
            "Toggle Git/SFTP mode on dev or a multidev (to Git asks: uncommitted SFTP changes are lost)",
        ),
        key("w", "wake", "Wake a sleeping environment"),
        key("r", "refresh", "Reload environments from Pantheon"),
        key(
            "Esc",
            "",
            "Clear an active filter; otherwise back to the sites list",
        ),
        key("q", "back", "Back to the sites list"),
    ],
};

pub static BACKUPS: Section = Section {
    title: "Backups",
    keys: &[
        key(
            "c",
            "create backup",
            "Create a backup of code, database and files",
        ),
        key(
            "y",
            "copy file",
            "Copy the backup filename to the clipboard",
        ),
        key("r", "refresh", "Reload backups from Pantheon"),
        key("q / Esc", "back", "Back to the environments"),
    ],
};

// ── shared ────────────────────────────────────────────────────────────────────

pub static NAVIGATION: Section = Section {
    title: "Moving in lists",
    keys: &[
        key("j / ↓", "", "Next row"),
        key("k / ↑", "", "Previous row"),
        key("g / Home", "", "First row"),
        key("G / End", "", "Last row"),
    ],
};

pub static GLOBAL: Section = Section {
    title: "Everywhere (except login)",
    keys: &[
        key("h / ?", "help", "Show this help"),
        key(":", "command", "Open the command bar"),
        key(
            "l",
            "log",
            "Show / hide the output log (every terminus command and result)",
        ),
        key("PgUp / PgDn", "", "Scroll the output log while it is open"),
        key(
            "Esc",
            "",
            "Dismiss the current message; if there is none, go back",
        ),
        key("Ctrl-c", "", "Quit immediately (also on the login screen)"),
    ],
};

pub static FILTER: Section = Section {
    title: "While typing a / filter",
    keys: &[
        key(
            "any text",
            "type",
            "Narrow the list (case-insensitive, matches anywhere)",
        ),
        key("Backspace", "", "Delete the last character"),
        key(
            "↑ / ↓",
            "move",
            "Move through the matches without leaving the filter",
        ),
        key("Enter", "keep", "Stop typing and keep the filter"),
        key("Esc", "clear", "Stop typing and clear the filter"),
    ],
};

pub static NEW_MULTIDEV: Section = Section {
    title: "New multidev dialog",
    keys: &[
        key(
            "any text",
            "name",
            "Multidev name: max 11 chars, lowercase letters, numbers and hyphens",
        ),
        key("Backspace", "", "Delete the last character"),
        key(
            "Tab / →",
            "next source",
            "Next source environment for database & files (dev, test, live)",
        ),
        key("Shift-Tab / ←", "", "Previous source environment"),
        key("Enter", "create", "Create it (only when the name is valid)"),
        key("Esc", "cancel", "Close without creating"),
    ],
};

pub static DELETE_MULTIDEV: Section = Section {
    title: "Delete multidev dialog",
    keys: &[
        key(
            "any text",
            "type name",
            "Type the multidev's name to unlock deletion",
        ),
        key("Backspace", "", "Delete the last character"),
        key(
            "Tab",
            "branch",
            "Toggle deleting its git branch too (on by default)",
        ),
        key(
            "Enter",
            "delete",
            "Delete it (only when the typed name matches)",
        ),
        key("Esc", "cancel", "Close without deleting"),
    ],
};

pub static CONFIRM: Section = Section {
    title: "Yes/no confirmations (deploy, switch to Git, logout)",
    keys: &[
        key("y", "confirm", "Confirm"),
        key("any other key", "cancel", "Cancel"),
    ],
};

pub static COMMAND_BAR: Section = Section {
    title: "Command bar (:)",
    keys: &[
        key("Enter", "run", "Run the command"),
        key("Backspace", "", "Delete the last character"),
        key("Esc", "cancel", "Close the command bar"),
    ],
};

pub static COMMANDS: Section = Section {
    title: "Commands",
    keys: &[
        key(":q  :quit", "", "Quit"),
        key(":d  :dashboard", "", "Go to the dashboard"),
        key(":s  :sites", "", "Go to the sites list"),
        key(":select", "", "Open the site selector"),
        key(":r  :refresh", "", "Reload the current screen"),
        key(
            ":site <name>",
            "",
            "Open a site: exact name, or a unique partial match; several matches filter the list",
        ),
        key(
            ":multidev:create [name]",
            "",
            "Open the new-multidev dialog (environments/backups screens)",
        ),
        key(
            ":multidev:delete <name>",
            "",
            "Open the delete dialog for that multidev (environments/backups screens)",
        ),
        key(
            ":logout",
            "",
            "Log out of Terminus, forget the stored token and clear the cache (asks y/N)",
        ),
    ],
};

pub static HELP: Section = Section {
    title: "This help",
    keys: &[
        key("j / k  ↓ / ↑", "", "Scroll one line"),
        key("PgDn / Space  PgUp", "", "Scroll one page"),
        key("g / G", "", "Top / bottom"),
        key("Esc / q / h / ? / Enter", "", "Close"),
    ],
};

/// Every screen, in the order the app flows.
pub static SCREENS: [(Screen, &Section); 6] = [
    (Screen::Login, &LOGIN),
    (Screen::Dashboard, &DASHBOARD),
    (Screen::Sites, &SITES),
    (Screen::SiteSelector, &SELECTOR),
    (Screen::Environments, &ENVIRONMENTS),
    (Screen::Backups, &BACKUPS),
];

pub fn screen_section(screen: Screen) -> &'static Section {
    SCREENS
        .iter()
        .find(|(s, _)| *s == screen)
        .map(|(_, section)| *section)
        .unwrap_or(&DASHBOARD)
}

fn is_list(screen: Screen) -> bool {
    matches!(
        screen,
        Screen::Sites | Screen::SiteSelector | Screen::Environments | Screen::Backups
    )
}

/// Footer hints for the current state, most specific first.
pub fn footer_hints(app: &App) -> Vec<(&'static str, &'static str)> {
    let from = |section: &Section| -> Vec<(&'static str, &'static str)> {
        section
            .keys
            .iter()
            .filter(|k| !k.hint.is_empty())
            .map(|k| (k.keys, k.hint))
            .collect()
    };

    let filter_editing = match app.screen {
        Screen::Sites => app.sites_filter.editing,
        Screen::SiteSelector => app.selector_filter.editing,
        Screen::Environments => app.envs_filter.editing,
        _ => false,
    };
    if filter_editing {
        return from(&FILTER);
    }
    if let Overlay::Command = app.overlay {
        return from(&COMMAND_BAR);
    }

    let mut hints = vec![];
    if is_list(app.screen) {
        hints.push(("j/k", "move"));
    }
    let mut screen = from(screen_section(app.screen));
    if app.screen == Screen::SiteSelector && app.selector_first_run {
        screen.retain(|(keys, _)| *keys != "q / Esc");
        screen.push(("Ctrl-c", "quit"));
    }
    hints.extend(screen);
    if app.screen != Screen::Login {
        hints.extend(from(&GLOBAL));
    }
    hints
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [&Section; 15] = [
        &LOGIN,
        &DASHBOARD,
        &SITES,
        &SELECTOR,
        &ENVIRONMENTS,
        &BACKUPS,
        &NAVIGATION,
        &GLOBAL,
        &FILTER,
        &NEW_MULTIDEV,
        &DELETE_MULTIDEV,
        &CONFIRM,
        &COMMAND_BAR,
        &COMMANDS,
        &HELP,
    ];

    #[test]
    fn every_screen_has_a_section_with_hints() {
        for (screen, section) in SCREENS {
            assert!(std::ptr::eq(screen_section(screen), section));
            assert!(section.keys.iter().any(|k| !k.hint.is_empty()));
        }
    }

    #[test]
    fn every_binding_is_documented() {
        for section in ALL {
            for k in section.keys {
                assert!(
                    !k.keys.is_empty() && !k.help.is_empty(),
                    "{}",
                    section.title
                );
            }
        }
    }

    #[test]
    fn every_command_is_documented() {
        let documented: String = COMMANDS.keys.iter().map(|k| k.keys).collect();
        for cmd in [
            "q",
            "quit",
            "d",
            "dashboard",
            "s",
            "sites",
            "select",
            "r",
            "refresh",
            "site",
            "multidev:create",
            "multidev:delete",
            "logout",
        ] {
            assert!(documented.contains(&format!(":{cmd}")), ":{cmd} missing");
        }
    }
}
