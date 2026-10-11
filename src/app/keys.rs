//! Keyboard handling: overlays, dialogs, per-screen keys and the command bar.
//! Every key documented in `ui::keymap` is handled here — keep them in sync.

use super::*;
use crate::{model::PROTECTED_ENVS, system};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Lines moved by PgUp/PgDn in the help overlay.
const HELP_PAGE: u16 = 10;

impl App {
    pub fn handle_key(&mut self, key: KeyEvent) {
        match self.overlay.clone() {
            Overlay::Help => return self.handle_help(key),
            Overlay::Command => return self.handle_command_input(key),
            Overlay::Confirm(_, action) => {
                self.overlay = Overlay::None;
                if matches!(key.code, KeyCode::Char('y' | 'Y')) {
                    self.execute_confirm(action);
                }
                return;
            }
            Overlay::NewMultidev(form) => return self.handle_new_multidev(key, form),
            Overlay::DeleteMultidev(form) => return self.handle_delete_multidev(key, form),
            Overlay::None => {}
        }

        // While typing into a filter every key belongs to the filter.
        if self.active_filter().is_some_and(|f| f.editing) {
            self.handle_filter_input(key);
            return;
        }

        if self.screen != Screen::Login {
            match key.code {
                KeyCode::Char('h' | '?') => {
                    self.overlay = Overlay::Help;
                    self.help_scroll = 0;
                    return;
                }
                KeyCode::Char(':') => {
                    self.overlay = Overlay::Command;
                    self.command_input.clear();
                    return;
                }
                KeyCode::Char('l') => {
                    self.show_log = !self.show_log;
                    self.log_scroll = 0;
                    return;
                }
                KeyCode::PageUp if self.show_log => {
                    self.log_scroll = (self.log_scroll + LOG_SCROLL_STEP)
                        .min(self.log_entries.len().saturating_sub(1));
                    return;
                }
                KeyCode::PageDown if self.show_log => {
                    self.log_scroll = self.log_scroll.saturating_sub(LOG_SCROLL_STEP);
                    return;
                }
                // First Esc dismisses a pending message; otherwise it falls
                // through to the screen handler (which navigates back).
                KeyCode::Esc if self.error.is_some() || self.status.is_some() => {
                    self.error = None;
                    self.status = None;
                    return;
                }
                _ => {}
            }
        }

        match self.screen {
            Screen::Login => self.handle_login(key),
            Screen::Dashboard => self.handle_dashboard(key),
            Screen::Sites => self.handle_sites(key),
            Screen::Environments => self.handle_environments(key),
            Screen::Backups => self.handle_backups(key),
            Screen::SiteSelector => self.handle_selector(key),
        }
    }

    // ── overlays ──────────────────────────────────────────────────────────────

    fn handle_help(&mut self, key: KeyEvent) {
        let max = self.help_max_scroll.get();
        self.help_scroll = match key.code {
            KeyCode::Down | KeyCode::Char('j') => self.help_scroll.saturating_add(1),
            KeyCode::Up | KeyCode::Char('k') => self.help_scroll.saturating_sub(1),
            KeyCode::PageDown | KeyCode::Char(' ') => self.help_scroll.saturating_add(HELP_PAGE),
            KeyCode::PageUp => self.help_scroll.saturating_sub(HELP_PAGE),
            KeyCode::Home | KeyCode::Char('g') => 0,
            KeyCode::End | KeyCode::Char('G') => max,
            KeyCode::Esc | KeyCode::Char('q' | 'h' | '?') | KeyCode::Enter => {
                self.overlay = Overlay::None;
                return;
            }
            _ => return,
        }
        .min(max);
    }

    fn handle_new_multidev(&mut self, key: KeyEvent, mut form: MultidevForm) {
        let count = form.sources.len().max(1);
        match key.code {
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                return;
            }
            KeyCode::Enter => {
                if form.problem().is_none() {
                    self.overlay = Overlay::None;
                    self.create_multidev(form);
                }
                return;
            }
            KeyCode::Tab | KeyCode::Right => form.source = (form.source + 1) % count,
            KeyCode::BackTab | KeyCode::Left => form.source = (form.source + count - 1) % count,
            KeyCode::Backspace => {
                form.name.pop();
            }
            KeyCode::Char(c) if is_text_input(&key) => form.name.push(c),
            _ => return,
        }
        self.overlay = Overlay::NewMultidev(form);
    }

    fn handle_delete_multidev(&mut self, key: KeyEvent, mut form: DeleteForm) {
        match key.code {
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                return;
            }
            KeyCode::Enter => {
                if form.confirmed() {
                    self.overlay = Overlay::None;
                    self.delete_multidev(form);
                }
                return;
            }
            KeyCode::Tab => form.delete_branch = !form.delete_branch,
            KeyCode::Backspace => {
                form.typed.pop();
            }
            KeyCode::Char(c) if is_text_input(&key) => form.typed.push(c),
            _ => return,
        }
        self.overlay = Overlay::DeleteMultidev(form);
    }

    fn active_filter(&self) -> Option<&ListFilter> {
        match self.screen {
            Screen::Sites => Some(&self.sites_filter),
            Screen::SiteSelector => Some(&self.selector_filter),
            Screen::Environments => Some(&self.envs_filter),
            _ => None,
        }
    }

    fn handle_filter_input(&mut self, key: KeyEvent) {
        let filter = match self.screen {
            Screen::Sites => &mut self.sites_filter,
            Screen::SiteSelector => &mut self.selector_filter,
            Screen::Environments => &mut self.envs_filter,
            _ => return,
        };
        match key.code {
            KeyCode::Enter => filter.editing = false,
            KeyCode::Esc => {
                filter.editing = false;
                filter.text.clear();
            }
            KeyCode::Backspace => {
                filter.text.pop();
            }
            KeyCode::Char(c) if is_text_input(&key) => filter.text.push(c),
            KeyCode::Up | KeyCode::Down => {
                // Allow moving through the matches without leaving the filter.
                match self.screen {
                    Screen::Sites => {
                        let len = self.listed_sites().len();
                        navigate(&mut self.sites_state, len, key.code);
                    }
                    Screen::SiteSelector => {
                        let len = self.selector_sites().len();
                        navigate(&mut self.selector_state, len, key.code);
                    }
                    Screen::Environments => {
                        let len = self.listed_environments().len();
                        navigate(&mut self.envs_state, len, key.code);
                    }
                    _ => {}
                }
                return;
            }
            _ => return,
        }
        self.reset_list_selections();
    }

    // ── per-screen key handlers ───────────────────────────────────────────────

    fn handle_login(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if is_text_input(&key) => {
                self.token_input.push(c);
                self.error = None;
            }
            KeyCode::Backspace => {
                self.token_input.pop();
            }
            KeyCode::Esc => {
                self.token_input.clear();
                self.error = None;
            }
            KeyCode::Enter if !self.token_input.trim().is_empty() && !self.is_loading() => {
                self.token_input = self.token_input.trim().to_string();
                let token = self.token_input.clone();
                self.error = None;
                self.spawn("terminus auth:login --machine-token=***", async move {
                    AppEvent::Login(terminus::login(&token).await.map_err(err_string))
                });
            }
            _ => {}
        }
    }

    fn handle_dashboard(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('s') | KeyCode::Enter => self.screen = Screen::Sites,
            KeyCode::Char('S') => self.open_selector(false),
            KeyCode::Char('r') => self.load_sites(),
            KeyCode::Char('q') => self.should_quit = true,
            _ => {}
        }
    }

    fn handle_sites(&mut self, key: KeyEvent) {
        let listed = self.listed_sites().len();
        if navigate(&mut self.sites_state, listed, key.code) {
            return;
        }
        match key.code {
            KeyCode::Esc if !self.sites_filter.text.is_empty() => {
                self.sites_filter.text.clear();
                self.reset_list_selections();
            }
            KeyCode::Char('q') | KeyCode::Esc => self.screen = Screen::Dashboard,
            KeyCode::Char('/') => self.sites_filter.editing = true,
            KeyCode::Enter => {
                let selected = self
                    .sites_state
                    .selected()
                    .and_then(|i| self.listed_sites().get(i).map(|s| s.name.clone()));
                if let Some(name) = selected {
                    self.open_site_envs(&name);
                }
            }
            KeyCode::Char('S') => self.open_selector(false),
            KeyCode::Char('r') => self.load_sites(),
            _ => {}
        }
    }

    fn handle_environments(&mut self, key: KeyEvent) {
        let listed = self.listed_environments().len();
        if navigate(&mut self.envs_state, listed, key.code) {
            return;
        }
        match key.code {
            KeyCode::Esc if !self.envs_filter.text.is_empty() => {
                self.envs_filter.text.clear();
                self.reset_list_selections();
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = Screen::Sites;
                self.current_site = None;
                self.environments.clear();
                self.envs_filter = ListFilter::default();
            }
            KeyCode::Char('/') => self.envs_filter.editing = true,
            KeyCode::Char('n') => self.open_new_multidev(String::new()),
            KeyCode::Char('r') => self.reload_envs(),
            KeyCode::Char('y') => {
                if let Some((site, env)) = self.selected_env() {
                    let target = env.url().unwrap_or_else(|| format!("{site}.{}", env.id));
                    match system::copy_to_clipboard(&target) {
                        Ok(msg) => self.set_status(msg),
                        Err(e) => self.set_error(format!("Could not copy to clipboard: {e}")),
                    }
                }
            }
            KeyCode::Char(c @ ('c' | 'd' | 'D' | 'm' | 'w' | 'o' | 'b')) => {
                let Some((site, env)) = self.selected_env() else {
                    return;
                };
                self.env_action(c, site, env);
            }
            _ => {}
        }
    }

    fn env_action(&mut self, key: char, site: String, env: Environment) {
        let target = format!("{site}.{}", env.id);
        match key {
            'c' => {
                let id = env.id.clone();
                self.spawn(format!("terminus env:clear-cache {target}"), async move {
                    AppEvent::Op {
                        what: "Cache clear".into(),
                        result: terminus::clear_cache(&site, &id).await.map_err(err_string),
                        refresh: Refresh::Nothing,
                    }
                });
            }
            'd' => self.open_delete_multidev(&env.id),
            'D' => match env.deploy_source() {
                Some(from) => {
                    let upper = if env.id == "live" { "LIVE" } else { "test" };
                    self.overlay = Overlay::Confirm(
                        format!("Deploy code from {from} to {upper} on '{site}'?"),
                        ConfirmAction::Deploy { site, env: env.id },
                    );
                }
                None => self.set_error("Deploys only target test (from dev) or live (from test)."),
            },
            'm' if env.id == "test" || env.id == "live" => {
                self.set_error("Connection mode can only be changed on dev and multidevs.");
            }
            'm' if env.is_sftp() => {
                self.overlay = Overlay::Confirm(
                    format!(
                        "Switch {target} to Git mode? Uncommitted SFTP changes will be discarded."
                    ),
                    ConfirmAction::SetConnection {
                        site,
                        env: env.id,
                        mode: "git",
                    },
                );
            }
            'm' => self.set_connection(site, env.id, "sftp"),
            'w' => {
                let id = env.id.clone();
                self.spawn(format!("terminus env:wake {target}"), async move {
                    AppEvent::Op {
                        what: "Wake".into(),
                        result: terminus::wake(&site, &id).await.map_err(err_string),
                        refresh: Refresh::Nothing,
                    }
                });
            }
            'o' => match env.url() {
                Some(url) => {
                    self.spawn(format!("open {url}"), async move {
                        AppEvent::Op {
                            what: "Open in browser".into(),
                            result: system::open_url(&url).await.map_err(err_string),
                            refresh: Refresh::Nothing,
                        }
                    });
                }
                None => self.set_error(format!("{target} has no domain.")),
            },
            'b' => self.open_backups(env.id),
            _ => {}
        }
    }

    fn handle_backups(&mut self, key: KeyEvent) {
        if navigate(&mut self.backups_state, self.backups.len(), key.code) {
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.screen = Screen::Environments;
                self.current_env = None;
                self.backups.clear();
            }
            KeyCode::Char('r') => self.reload_backups(),
            KeyCode::Char('y') => {
                if let Some(i) = self.backups_state.selected()
                    && let Some(b) = self.backups.get(i)
                {
                    match system::copy_to_clipboard(&b.file) {
                        Ok(msg) => self.set_status(msg),
                        Err(e) => self.set_error(format!("Could not copy to clipboard: {e}")),
                    }
                }
            }
            KeyCode::Char('c') => {
                let (Some(site), Some(env)) = (self.current_site.clone(), self.current_env.clone())
                else {
                    return;
                };
                self.spawn(format!("terminus backup:create {site}.{env}"), async move {
                    AppEvent::Op {
                        what: "Backup".into(),
                        result: terminus::create_backup(&site, &env)
                            .await
                            .map_err(err_string),
                        refresh: Refresh::Backups(site, env),
                    }
                });
            }
            _ => {}
        }
    }

    fn handle_selector(&mut self, key: KeyEvent) {
        let len = self.selector_sites().len();
        if navigate(&mut self.selector_state, len, key.code) {
            return;
        }
        match key.code {
            KeyCode::Char(' ') => {
                let name = self
                    .selector_state
                    .selected()
                    .and_then(|i| self.selector_sites().get(i).map(|s| s.name.clone()));
                if let Some(name) = name
                    && !self.selector_checked.remove(&name)
                {
                    self.selector_checked.insert(name);
                }
            }
            KeyCode::Char('a') => {
                let names: Vec<String> = self
                    .selector_sites()
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();
                self.selector_checked.extend(names);
            }
            KeyCode::Char('n') => {
                let names: Vec<String> = self
                    .selector_sites()
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();
                for name in names {
                    self.selector_checked.remove(&name);
                }
            }
            KeyCode::Char('/') => self.selector_filter.editing = true,
            KeyCode::Esc if !self.selector_filter.text.is_empty() => {
                self.selector_filter.text.clear();
                self.reset_list_selections();
            }
            KeyCode::Char('r') => self.load_sites(),
            KeyCode::Enter => self.save_selection(),
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.selector_first_run {
                    self.set_error(
                        "Pick at least one site and press Enter to finish setup (Ctrl-c quits).",
                    );
                } else {
                    self.screen = self.selector_return;
                }
            }
            _ => {}
        }
    }

    // ── site selector ─────────────────────────────────────────────────────────

    pub(super) fn open_selector(&mut self, first_run: bool) {
        if self.screen != Screen::SiteSelector {
            self.selector_return = match self.screen {
                Screen::Sites => Screen::Sites,
                _ => Screen::Dashboard,
            };
        }
        self.selector_first_run = first_run;
        self.selector_checked = self
            .config
            .selected_sites
            .iter()
            .flatten()
            .filter(|name| !self.sites_loaded || self.sites.iter().any(|s| &s.name == *name))
            .cloned()
            .collect();
        self.selector_filter = ListFilter::default();
        self.screen = Screen::SiteSelector;
        self.reset_list_selections();
        if !self.sites_loaded && !self.is_loading() {
            self.load_sites();
        }
    }

    fn save_selection(&mut self) {
        // Drop sites that no longer exist so the saved list doesn't rot.
        if self.sites_loaded {
            let existing: BTreeSet<&str> = self.sites.iter().map(|s| s.name.as_str()).collect();
            self.selector_checked
                .retain(|name| existing.contains(name.as_str()));
        }
        if self.selector_checked.is_empty() {
            self.set_error("Select at least one site (press a to select all).");
            return;
        }
        self.config.selected_sites = Some(self.selector_checked.iter().cloned().collect());
        if !self.save_config() {
            return;
        }
        let shown = self.visible_sites().len();
        self.set_status(format!(
            "Showing {shown} of {} sites. Press S to change it.",
            self.sites.len()
        ));
        self.selector_first_run = false;
        self.sites_filter = ListFilter::default();
        self.screen = self.selector_return;
        self.reset_list_selections();
    }

    // ── multidev dialogs ──────────────────────────────────────────────────────

    fn open_new_multidev(&mut self, name: String) {
        let Some(site) = self.current_site.clone() else {
            self.set_error("Open a site's environments first.");
            return;
        };
        // Database and files can only be cloned from an existing, initialized
        // dev/test/live environment.
        let mut sources: Vec<String> = PROTECTED_ENVS
            .iter()
            .filter(|id| {
                self.environments
                    .iter()
                    .any(|e| e.id == **id && e.initialized != Some(false))
            })
            .map(|id| id.to_string())
            .collect();
        if sources.is_empty() {
            sources.push("dev".into());
        }
        let highlighted = self.selected_env_id();
        let source = highlighted
            .and_then(|id| sources.iter().position(|s| *s == id))
            .unwrap_or(0);
        self.overlay = Overlay::NewMultidev(MultidevForm {
            site,
            name,
            sources,
            source,
            taken: self.environments.iter().map(|e| e.id.clone()).collect(),
        });
    }

    fn open_delete_multidev(&mut self, env: &str) {
        let Some(site) = self.current_site.clone() else {
            self.set_error("Open a site's environments first.");
            return;
        };
        if PROTECTED_ENVS.contains(&env) {
            self.set_error("Pantheon doesn't allow deleting dev, test or live — only multidevs.");
            return;
        }
        if !self.environments.iter().any(|e| e.id == env) {
            self.set_error(format!("'{env}' is not an environment of {site}."));
            return;
        }
        self.overlay = Overlay::DeleteMultidev(DeleteForm {
            site,
            env: env.to_string(),
            typed: String::new(),
            delete_branch: true,
        });
    }

    fn create_multidev(&mut self, form: MultidevForm) {
        let MultidevForm { site, name, .. } = form.clone();
        let from = form.source_env().to_string();
        self.spawn(
            format!("terminus multidev:create {site}.{from} {name}"),
            async move {
                AppEvent::Op {
                    what: "Multidev create".into(),
                    result: terminus::create_multidev(&site, &from, &name)
                        .await
                        .map_err(err_string),
                    refresh: Refresh::Envs(site),
                }
            },
        );
    }

    fn delete_multidev(&mut self, form: DeleteForm) {
        let DeleteForm {
            site,
            env,
            delete_branch,
            ..
        } = form;
        let branch_flag = if delete_branch {
            " --delete-branch"
        } else {
            ""
        };
        self.spawn(
            format!("terminus multidev:delete {site}.{env}{branch_flag} --yes"),
            async move {
                AppEvent::Op {
                    what: "Multidev delete".into(),
                    result: terminus::delete_multidev(&site, &env, delete_branch)
                        .await
                        .map_err(err_string),
                    refresh: Refresh::Envs(site),
                }
            },
        );
    }

    // ── command bar ───────────────────────────────────────────────────────────

    fn handle_command_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.overlay = Overlay::None;
                self.command_input.clear();
            }
            KeyCode::Backspace => {
                self.command_input.pop();
            }
            KeyCode::Enter => {
                let cmd = self.command_input.trim().to_string();
                self.overlay = Overlay::None;
                self.command_input.clear();
                self.execute_command(&cmd);
            }
            KeyCode::Char(c) if is_text_input(&key) => self.command_input.push(c),
            _ => {}
        }
    }

    pub(super) fn execute_command(&mut self, cmd: &str) {
        let (name, arg) = match cmd.split_once(' ') {
            Some((name, arg)) => (name, Some(arg.trim()).filter(|a| !a.is_empty())),
            None => (cmd, None),
        };
        let on_env_screens = matches!(self.screen, Screen::Environments | Screen::Backups);
        match (name, arg) {
            ("", None) => {}
            ("q" | "quit", None) => self.should_quit = true,
            ("dashboard" | "d", None) => self.screen = Screen::Dashboard,
            ("sites" | "s", None) => self.screen = Screen::Sites,
            ("select", None) => self.open_selector(false),
            ("refresh" | "r", None) => self.refresh_current(),
            ("logout", None) => {
                self.overlay = Overlay::Confirm(
                    "Log out? This also removes Terminus' saved session and sunim's cache.".into(),
                    ConfirmAction::Logout,
                );
            }
            ("site", Some(query)) => self.jump_to_site(query),
            ("multidev:create" | "multidev:delete", _) if !on_env_screens => {
                self.set_error("Open a site's environments first.");
            }
            ("multidev:create", arg) => {
                self.screen = Screen::Environments;
                self.open_new_multidev(arg.unwrap_or_default().to_string());
            }
            ("multidev:delete", Some(env)) => {
                self.screen = Screen::Environments;
                self.open_delete_multidev(env);
            }
            ("site" | "multidev:delete", None) => {
                self.set_error(format!("Usage: :{name} <name>"));
            }
            _ => self.set_error(format!("Unknown command: '{cmd}'. Press 'h' for help.")),
        }
    }

    /// What `r` does on the current screen.
    fn refresh_current(&mut self) {
        match self.screen {
            Screen::Login => {}
            Screen::Dashboard | Screen::Sites | Screen::SiteSelector => self.load_sites(),
            Screen::Environments => self.reload_envs(),
            Screen::Backups => self.reload_backups(),
        }
    }

    /// Exact name first; otherwise a unique partial match among visible sites.
    /// Several matches open the sites list filtered by the query.
    fn jump_to_site(&mut self, query: &str) {
        if !self.sites_loaded {
            self.set_error("Sites are still loading, try again in a moment.");
            return;
        }
        if let Some(site) = self.sites.iter().find(|s| s.name == query) {
            let name = site.name.clone();
            self.open_site_envs(&name);
            return;
        }
        let matches: Vec<String> = self
            .visible_sites()
            .iter()
            .filter(|s| model::matches_filter(&s.name, query))
            .map(|s| s.name.clone())
            .collect();
        match matches.as_slice() {
            [] => self.set_error(format!("No site matches '{query}'.")),
            [only] => {
                let name = only.clone();
                self.open_site_envs(&name);
            }
            _ => {
                self.sites_filter = ListFilter {
                    text: query.to_string(),
                    editing: false,
                };
                self.screen = Screen::Sites;
                self.reset_list_selections();
                self.set_status(format!("{} sites match '{query}'.", matches.len()));
            }
        }
    }

    fn execute_confirm(&mut self, action: ConfirmAction) {
        match action {
            ConfirmAction::Deploy { site, env } => {
                self.spawn(
                    format!(
                        "terminus env:deploy {site}.{env} --note=\"{}\"",
                        terminus::DEPLOY_NOTE
                    ),
                    async move {
                        AppEvent::Op {
                            what: "Deploy".into(),
                            result: terminus::deploy(&site, &env).await.map_err(err_string),
                            refresh: Refresh::Envs(site),
                        }
                    },
                );
            }
            ConfirmAction::SetConnection { site, env, mode } => {
                self.set_connection(site, env, mode);
            }
            ConfirmAction::Logout => {
                self.spawn("terminus auth:logout", async {
                    AppEvent::LoggedOut(terminus::logout().await.map_err(err_string))
                });
            }
        }
    }

    fn set_connection(&mut self, site: String, env: String, mode: &'static str) {
        self.spawn(
            format!("terminus connection:set {site}.{env} {mode}"),
            async move {
                AppEvent::Op {
                    what: "Connection mode change".into(),
                    result: terminus::set_connection_mode(&site, &env, mode)
                        .await
                        .map_err(err_string),
                    refresh: Refresh::Envs(site),
                }
            },
        );
    }
}

/// Plain characters only: Ctrl/Alt combos must not end up in text fields.
fn is_text_input(key: &KeyEvent) -> bool {
    !key.modifiers
        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
}

/// Moves the list cursor for navigation keys; returns whether the key was one.
pub(super) fn navigate(state: &mut ListState, len: usize, code: KeyCode) -> bool {
    let is_nav = matches!(
        code,
        KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Char('k' | 'j' | 'g' | 'G')
    );
    if !is_nav {
        return false;
    }
    if len == 0 {
        state.select(None);
        return true;
    }
    let current = state.selected().unwrap_or(0).min(len - 1);
    let next = match code {
        KeyCode::Up | KeyCode::Char('k') => current.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => (current + 1).min(len - 1),
        KeyCode::Home | KeyCode::Char('g') => 0,
        _ => len - 1,
    };
    state.select(Some(next));
    true
}
