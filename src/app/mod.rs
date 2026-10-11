//! Application state and behaviour. Rendering lives in `crate::ui`, which only
//! reads this state.
//!
//! - `mod.rs`: state, background tasks, results handling, cache.
//! - `keys.rs`: keyboard handling, dialogs and the command bar.

use crate::{
    model::{self, Backup, Environment, Site},
    storage::{self, Cache, Cached, Config},
    terminus,
};
use ratatui::widgets::ListState;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    future::Future,
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;

mod keys;
#[cfg(test)]
mod tests;

/// Oldest log entries are dropped past this point.
pub const MAX_LOG_ENTRIES: usize = 500;
const LOG_SCROLL_STEP: usize = 5;
/// Success messages fade out after this; errors stay until dismissed.
const STATUS_TTL: Duration = Duration::from_secs(5);

// ── screen / overlay state ────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Login,
    Dashboard,
    Sites,
    Environments,
    Backups,
    SiteSelector,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Overlay {
    None,
    Help,
    Command,
    Confirm(String, ConfirmAction),
    NewMultidev(MultidevForm),
    DeleteMultidev(DeleteForm),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    Deploy {
        site: String,
        env: String,
    },
    SetConnection {
        site: String,
        env: String,
        mode: &'static str,
    },
    Logout,
}

/// "New multidev" dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct MultidevForm {
    pub site: String,
    pub name: String,
    /// Environments the database and files can be cloned from.
    pub sources: Vec<String>,
    pub source: usize,
    /// Existing environment ids, to reject duplicates while typing.
    pub taken: Vec<String>,
}

impl MultidevForm {
    pub fn source_env(&self) -> &str {
        self.sources.get(self.source).map_or("dev", String::as_str)
    }

    /// Why the current name can't be used, if it can't.
    pub fn problem(&self) -> Option<String> {
        if let Err(e) = model::validate_multidev_name(&self.name) {
            return Some(e);
        }
        self.taken
            .contains(&self.name)
            .then(|| format!("'{}' already exists on {}.", self.name, self.site))
    }
}

/// "Delete multidev" dialog: the user must type the name to confirm.
#[derive(Debug, Clone, PartialEq)]
pub struct DeleteForm {
    pub site: String,
    pub env: String,
    pub typed: String,
    pub delete_branch: bool,
}

impl DeleteForm {
    pub fn confirmed(&self) -> bool {
        self.typed == self.env
    }
}

/// Text typed after `/` to narrow a list.
#[derive(Debug, Clone, Default)]
pub struct ListFilter {
    pub text: String,
    /// True while the user is typing into the filter.
    pub editing: bool,
}

// ── log ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Command,
    Info,
    Success,
    Error,
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

// ── background tasks ──────────────────────────────────────────────────────────

/// What to reload once a mutating operation finishes.
#[derive(Debug, Clone, PartialEq)]
pub enum Refresh {
    Nothing,
    Envs(String),
    Backups(String, String),
}

/// Result of a background task.
pub enum AppEvent {
    /// Startup check: existing terminus session, or re-login with the stored token.
    Session(Result<String, String>),
    /// Manual login from the login screen.
    Login(Result<String, String>),
    Sites(Result<Vec<Site>, String>),
    Envs {
        site: String,
        result: Result<Vec<Environment>, String>,
    },
    Backups {
        site: String,
        env: String,
        result: Result<Vec<Backup>, String>,
    },
    /// A mutating operation (cache clear, deploy…). `what` names it in errors.
    Op {
        what: String,
        result: Result<String, String>,
        refresh: Refresh,
    },
    LoggedOut(Result<(), String>),
}

/// What a task spawned by [`App::spawn`] sends back: exactly one per task.
pub struct TaskDone {
    id: u64,
    event: AppEvent,
}

// ── app ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub screen: Screen,
    pub overlay: Overlay,
    pub should_quit: bool,
    pub spinner_tick: u8,
    pub error: Option<String>,
    pub status: Option<String>,
    status_set_at: Option<Instant>,

    config: Config,
    config_path: Option<PathBuf>,
    cache: Cache,
    cache_path: Option<PathBuf>,
    tx: mpsc::UnboundedSender<TaskDone>,
    /// Running background tasks by id, with a short label for the UI.
    running: BTreeMap<u64, String>,
    next_task: u64,

    pub token_input: String,
    pub whoami: Option<String>,

    pub sites: Vec<Site>,
    pub sites_loaded: bool,
    /// When the shown sites were fetched (Unix seconds).
    pub sites_updated: Option<u64>,
    pub sites_state: ListState,
    pub sites_filter: ListFilter,

    pub selector_checked: BTreeSet<String>,
    pub selector_state: ListState,
    pub selector_filter: ListFilter,
    /// First-run setup can't be cancelled, only completed.
    pub selector_first_run: bool,
    selector_return: Screen,

    pub current_site: Option<String>,
    pub environments: Vec<Environment>,
    pub envs_updated: Option<u64>,
    pub envs_state: ListState,
    pub envs_filter: ListFilter,

    /// Environment whose backups are shown on the backups screen.
    pub current_env: Option<String>,
    pub backups: Vec<Backup>,
    pub backups_updated: Option<u64>,
    pub backups_state: ListState,

    pub command_input: String,

    pub help_scroll: u16,
    /// Largest useful `help_scroll`, written by the help overlay when drawn.
    pub help_max_scroll: Cell<u16>,

    pub log_entries: Vec<LogEntry>,
    pub show_log: bool,
    /// Lines scrolled up from the newest log entry (0 = follow the tail).
    pub log_scroll: usize,
}

impl App {
    pub fn new(
        config: Config,
        config_path: Option<PathBuf>,
        cache_path: Option<PathBuf>,
        tx: mpsc::UnboundedSender<TaskDone>,
    ) -> Self {
        let cache = cache_path
            .as_deref()
            .map(storage::load_cache)
            .unwrap_or_default();
        Self {
            screen: Screen::Login,
            overlay: Overlay::None,
            should_quit: false,
            spinner_tick: 0,
            error: None,
            status: None,
            status_set_at: None,
            config,
            config_path,
            cache,
            cache_path,
            tx,
            running: BTreeMap::new(),
            next_task: 0,
            token_input: String::new(),
            whoami: None,
            sites: vec![],
            sites_loaded: false,
            sites_updated: None,
            sites_state: ListState::default(),
            sites_filter: ListFilter::default(),
            selector_checked: BTreeSet::new(),
            selector_state: ListState::default(),
            selector_filter: ListFilter::default(),
            selector_first_run: false,
            selector_return: Screen::Dashboard,
            current_site: None,
            environments: vec![],
            envs_updated: None,
            envs_state: ListState::default(),
            envs_filter: ListFilter::default(),
            current_env: None,
            backups: vec![],
            backups_updated: None,
            backups_state: ListState::default(),
            command_input: String::new(),
            help_scroll: 0,
            help_max_scroll: Cell::new(0),
            log_entries: vec![],
            show_log: false,
            log_scroll: 0,
        }
    }

    /// Shows cached data right away (if any) and kicks off the session check.
    pub fn start(&mut self) {
        if !terminus::is_installed() {
            self.set_error(
                "`terminus` was not found in $PATH — install it: \
                 https://docs.pantheon.io/terminus/install",
            );
            return;
        }
        if self.cache.user.is_some() && self.cache.sites.is_some() {
            // Optimistic start: assume the session is still valid; the check
            // below sends us back to the login screen if it isn't.
            self.whoami = self.cache.user.clone();
            self.screen = Screen::Dashboard;
            self.apply_cached_sites();
        }
        let token = self.config.token.clone();
        self.token_input = token.clone().unwrap_or_default();
        self.spawn("terminus auth:whoami", async move {
            AppEvent::Session(terminus::restore_session(token).await.map_err(err_string))
        });
    }

    pub fn is_loading(&self) -> bool {
        !self.running.is_empty()
    }

    /// Label of the newest running task and how many others are running.
    pub fn activity(&self) -> Option<(&str, usize)> {
        self.running
            .values()
            .next_back()
            .map(|label| (label.as_str(), self.running.len() - 1))
    }

    /// When the data on the current screen was fetched (Unix seconds).
    pub fn data_updated(&self) -> Option<u64> {
        match self.screen {
            Screen::Login => None,
            Screen::Dashboard | Screen::Sites | Screen::SiteSelector => self.sites_updated,
            Screen::Environments => self.envs_updated,
            Screen::Backups => self.backups_updated,
        }
    }

    pub fn tick(&mut self) {
        if self.is_loading() {
            self.spinner_tick = (self.spinner_tick + 1) % 8;
        }
        if self
            .status_set_at
            .is_some_and(|at| at.elapsed() >= STATUS_TTL)
        {
            self.status = None;
            self.status_set_at = None;
        }
    }

    /// Sites the user chose to manage (all of them until a selection exists).
    pub fn visible_sites(&self) -> Vec<&Site> {
        match &self.config.selected_sites {
            Some(selected) => self
                .sites
                .iter()
                .filter(|s| selected.contains(&s.name))
                .collect(),
            None => self.sites.iter().collect(),
        }
    }

    /// Visible sites narrowed by the `/` filter of the sites screen.
    pub fn listed_sites(&self) -> Vec<&Site> {
        self.visible_sites()
            .into_iter()
            .filter(|s| model::matches_filter(&s.name, &self.sites_filter.text))
            .collect()
    }

    /// All sites narrowed by the `/` filter of the selector.
    pub fn selector_sites(&self) -> Vec<&Site> {
        self.sites
            .iter()
            .filter(|s| model::matches_filter(&s.name, &self.selector_filter.text))
            .collect()
    }

    /// Environments narrowed by the `/` filter of the environments screen.
    pub fn listed_environments(&self) -> Vec<&Environment> {
        self.environments
            .iter()
            .filter(|e| model::matches_filter(&e.id, &self.envs_filter.text))
            .collect()
    }

    pub fn has_site_selection(&self) -> bool {
        self.config.selected_sites.is_some()
    }

    pub fn push_log(&mut self, level: LogLevel, msg: impl Into<String>) {
        self.log_entries.push(LogEntry {
            level,
            message: msg.into(),
        });
        if self.log_entries.len() > MAX_LOG_ENTRIES {
            let excess = self.log_entries.len() - MAX_LOG_ENTRIES;
            self.log_entries.drain(..excess);
        }
        // Keep the view anchored while the user is reading older entries.
        if self.log_scroll > 0 {
            self.log_scroll = (self.log_scroll + 1).min(self.log_entries.len().saturating_sub(1));
        }
    }

    fn set_error(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.push_log(LogLevel::Error, &msg);
        self.status = None;
        self.status_set_at = None;
        self.error = Some(msg);
    }

    fn set_status(&mut self, msg: impl Into<String>) {
        self.error = None;
        self.status = Some(msg.into());
        self.status_set_at = Some(Instant::now());
    }

    /// Runs `task` in the background. `log` is the command line shown in the
    /// log panel; a short form of it labels the task in the top-right corner.
    /// The result comes back as a [`TaskDone`] for [`App::handle_task_done`].
    fn spawn<F>(&mut self, log: impl Into<String>, task: F)
    where
        F: Future<Output = AppEvent> + Send + 'static,
    {
        let log = log.into();
        let id = self.next_task;
        self.next_task += 1;
        self.running.insert(id, short_label(&log));
        self.push_log(LogLevel::Command, log);
        if cfg!(test) {
            // Unit tests must never run terminus; they only observe `running`.
            drop(task);
            return;
        }
        let tx = self.tx.clone();
        tokio::spawn(async move {
            // A panicking task must still report back, or it would stay
            // "running" forever.
            let event = tokio::spawn(task).await.unwrap_or_else(|e| AppEvent::Op {
                what: "Background task".into(),
                result: Err(panic_message(e)),
                refresh: Refresh::Nothing,
            });
            let _ = tx.send(TaskDone { id, event });
        });
    }

    fn save_config(&mut self) -> bool {
        let Some(path) = self.config_path.clone() else {
            self.set_error("No config directory available; settings were not saved.");
            return false;
        };
        match storage::save(&path, &self.config) {
            Ok(()) => true,
            Err(e) => {
                self.set_error(format!("Could not save settings: {e:#}"));
                false
            }
        }
    }

    /// Cache problems are logged, never shown as errors: the cache is optional.
    fn save_cache(&mut self) {
        let Some(path) = self.cache_path.clone() else {
            return;
        };
        if let Err(e) = storage::save_cache(&path, &self.cache) {
            self.push_log(LogLevel::Error, format!("Could not save cache: {e:#}"));
        }
    }

    fn clear_cache(&mut self) {
        self.cache = Cache::default();
        if let Some(path) = self.cache_path.clone()
            && let Err(e) = storage::remove_cache(&path)
        {
            self.push_log(LogLevel::Error, format!("Could not remove cache: {e:#}"));
        }
    }

    fn apply_cached_sites(&mut self) {
        let Some(cached) = self.cache.sites.clone() else {
            return;
        };
        self.push_log(
            LogLevel::Info,
            format!(
                "Showing {} cached sites from {}",
                cached.items.len(),
                model::format_age(cached.fetched_at, model::now_unix())
            ),
        );
        self.sites = cached.items;
        self.sites_loaded = true;
        self.sites_updated = Some(cached.fetched_at);
        self.reset_list_selections();
        if self.config.selected_sites.is_none() {
            self.open_selector(true);
        }
    }

    // ── background results ────────────────────────────────────────────────────

    pub fn handle_task_done(&mut self, done: TaskDone) {
        self.running.remove(&done.id);
        self.handle_app_event(done.event);
    }

    fn handle_app_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::Session(Ok(user)) => self.logged_in(user),
            AppEvent::Session(Err(e)) => {
                self.push_log(LogLevel::Info, format!("No active session: {e}"));
                let was_optimistic = self.screen != Screen::Login;
                self.reset_session();
                if self.config.token.is_some() {
                    self.set_error(format!("Stored token was rejected: {e}"));
                } else if was_optimistic {
                    self.set_error("Your Terminus session has expired — sign in again.");
                }
            }
            AppEvent::Login(Ok(user)) => {
                // Only persist the token once terminus has accepted it.
                self.config.token = Some(self.token_input.clone());
                self.save_config();
                self.logged_in(user);
            }
            AppEvent::Login(Err(e)) => self.set_error(format!("Login failed: {e}")),
            AppEvent::Sites(Ok(sites)) => self.sites_loaded_from_terminus(sites),
            AppEvent::Sites(Err(e)) => self.set_error(format!("Failed to load sites: {e}")),
            AppEvent::Envs { site, result } => match result {
                Ok(envs) => {
                    let now = model::now_unix();
                    self.cache.envs.insert(
                        site.clone(),
                        Cached {
                            fetched_at: now,
                            items: envs.clone(),
                        },
                    );
                    self.save_cache();
                    if self.current_site.as_deref() != Some(site.as_str()) {
                        return;
                    }
                    self.push_log(
                        LogLevel::Info,
                        format!("Loaded {} environments of {site}", envs.len()),
                    );
                    let previous = self.selected_env_id();
                    self.environments = envs;
                    self.envs_updated = Some(now);
                    let ids: Vec<String> = self
                        .listed_environments()
                        .iter()
                        .map(|e| e.id.clone())
                        .collect();
                    reselect(&mut self.envs_state, &ids, previous.as_deref());
                }
                Err(e) if self.current_site.as_deref() == Some(site.as_str()) => {
                    self.set_error(format!("Failed to load environments: {e}"));
                }
                Err(e) => self.push_log(LogLevel::Error, format!("env:list {site}: {e}")),
            },
            AppEvent::Backups { site, env, result } => {
                let is_current = self.current_site.as_deref() == Some(site.as_str())
                    && self.current_env.as_deref() == Some(env.as_str());
                match result {
                    Ok(backups) => {
                        let now = model::now_unix();
                        self.cache.backups.insert(
                            format!("{site}.{env}"),
                            Cached {
                                fetched_at: now,
                                items: backups.clone(),
                            },
                        );
                        self.save_cache();
                        if !is_current {
                            return;
                        }
                        self.push_log(
                            LogLevel::Info,
                            format!("Loaded {} backups of {site}.{env}", backups.len()),
                        );
                        self.backups = backups;
                        self.backups_updated = Some(now);
                        keep_selection(&mut self.backups_state, self.backups.len());
                    }
                    Err(e) if is_current => self.set_error(format!("Failed to load backups: {e}")),
                    Err(e) => self.push_log(LogLevel::Error, format!("backup:list: {e}")),
                }
            }
            AppEvent::Op {
                what,
                result,
                refresh,
            } => match result {
                Ok(msg) => {
                    self.push_log(LogLevel::Success, &msg);
                    self.set_status(msg);
                    self.refresh(refresh);
                }
                Err(e) => self.set_error(format!("{what} failed: {e}")),
            },
            AppEvent::LoggedOut(result) => {
                match result {
                    Ok(()) => self.push_log(LogLevel::Success, "Logged out"),
                    Err(e) => self.push_log(LogLevel::Error, format!("auth:logout: {e}")),
                }
                self.config.token = None;
                self.save_config();
                self.clear_cache();
                self.reset_session();
            }
        }
    }

    fn logged_in(&mut self, user: String) {
        self.push_log(LogLevel::Success, format!("Authenticated as {user}"));
        self.adopt_user(&user);
        self.whoami = Some(user);
        if self.screen == Screen::Login {
            self.error = None;
            self.screen = Screen::Dashboard;
            if !self.sites_loaded {
                self.apply_cached_sites();
            }
        }
        self.load_sites();
    }

    /// Drops cached data that belongs to a different account.
    fn adopt_user(&mut self, user: &str) {
        if self.cache.user.as_deref().is_some_and(|u| u != user) {
            self.push_log(
                LogLevel::Info,
                "Cache belongs to another account; discarded",
            );
            self.clear_cache();
            self.sites.clear();
            self.sites_loaded = false;
            self.sites_updated = None;
            self.reset_list_selections();
        }
        self.cache.user = Some(user.to_string());
    }

    fn sites_loaded_from_terminus(&mut self, sites: Vec<Site>) {
        let now = model::now_unix();
        self.push_log(LogLevel::Info, format!("Loaded {} sites", sites.len()));

        let keep_listed = self.highlighted(&self.sites_state, &self.listed_sites());
        let keep_selector = self.highlighted(&self.selector_state, &self.selector_sites());

        self.sites = sites;
        self.sites_loaded = true;
        self.sites_updated = Some(now);

        // Forget cached environments/backups of sites that no longer exist.
        let names: BTreeSet<String> = self.sites.iter().map(|s| s.name.clone()).collect();
        self.cache.envs.retain(|site, _| names.contains(site));
        self.cache.backups.retain(|key, _| {
            key.split_once('.')
                .is_some_and(|(site, _)| names.contains(site))
        });
        self.cache.sites = Some(Cached {
            fetched_at: now,
            items: self.sites.clone(),
        });
        self.save_cache();

        let listed: Vec<String> = self.listed_sites().iter().map(|s| s.name.clone()).collect();
        reselect(&mut self.sites_state, &listed, keep_listed.as_deref());
        let selectable: Vec<String> = self
            .selector_sites()
            .iter()
            .map(|s| s.name.clone())
            .collect();
        reselect(
            &mut self.selector_state,
            &selectable,
            keep_selector.as_deref(),
        );

        if self.config.selected_sites.is_none()
            && !matches!(self.screen, Screen::SiteSelector | Screen::Login)
        {
            self.open_selector(true);
        }
    }

    fn highlighted(&self, state: &ListState, sites: &[&Site]) -> Option<String> {
        state
            .selected()
            .and_then(|i| sites.get(i))
            .map(|s| s.name.clone())
    }

    fn reset_session(&mut self) {
        self.screen = Screen::Login;
        self.overlay = Overlay::None;
        self.whoami = None;
        self.token_input = self.config.token.clone().unwrap_or_default();
        self.sites.clear();
        self.sites_loaded = false;
        self.sites_updated = None;
        self.sites_filter = ListFilter::default();
        self.current_site = None;
        self.environments.clear();
        self.envs_updated = None;
        self.envs_filter = ListFilter::default();
        self.current_env = None;
        self.backups.clear();
        self.backups_updated = None;
        self.reset_list_selections();
    }

    fn refresh(&mut self, refresh: Refresh) {
        match refresh {
            Refresh::Nothing => {}
            Refresh::Envs(site) => {
                if self.current_site.as_deref() == Some(site.as_str()) {
                    self.reload_envs();
                }
            }
            Refresh::Backups(site, env) => {
                if self.current_site.as_deref() == Some(site.as_str())
                    && self.current_env.as_deref() == Some(env.as_str())
                {
                    self.reload_backups();
                }
            }
        }
    }

    fn reset_list_selections(&mut self) {
        let listed = self.listed_sites().len();
        let selectable = self.selector_sites().len();
        let envs = self.listed_environments().len();
        select_first(&mut self.sites_state, listed);
        select_first(&mut self.selector_state, selectable);
        select_first(&mut self.envs_state, envs);
    }

    // ── loading data ──────────────────────────────────────────────────────────

    fn selected_env(&self) -> Option<(String, Environment)> {
        let site = self.current_site.clone()?;
        let env = (*self
            .listed_environments()
            .get(self.envs_state.selected()?)?)
        .clone();
        Some((site, env))
    }

    fn selected_env_id(&self) -> Option<String> {
        self.selected_env().map(|(_, env)| env.id)
    }

    fn load_sites(&mut self) {
        self.spawn("terminus site:list --format=json", async {
            AppEvent::Sites(terminus::site_list().await.map_err(err_string))
        });
    }

    fn reload_envs(&mut self) {
        let Some(site) = self.current_site.clone() else {
            return;
        };
        self.spawn(
            format!("terminus env:list {site} --format=json"),
            async move {
                let result = terminus::env_list(&site).await.map_err(err_string);
                AppEvent::Envs { site, result }
            },
        );
    }

    fn reload_backups(&mut self) {
        let (Some(site), Some(env)) = (self.current_site.clone(), self.current_env.clone()) else {
            return;
        };
        self.spawn(
            format!("terminus backup:list {site}.{env} --format=json"),
            async move {
                let result = terminus::backup_list(&site, &env).await.map_err(err_string);
                AppEvent::Backups { site, env, result }
            },
        );
    }

    /// Shows the cached environments (if any) at once and refreshes them.
    fn open_site_envs(&mut self, site_name: &str) {
        self.current_site = Some(site_name.to_string());
        self.current_env = None;
        self.screen = Screen::Environments;
        let cached = self.cache.envs.get(site_name).cloned();
        self.envs_updated = cached.as_ref().map(|c| c.fetched_at);
        self.environments = cached.map(|c| c.items).unwrap_or_default();
        self.envs_filter = ListFilter::default();
        self.envs_state = ListState::default();
        let count = self.listed_environments().len();
        select_first(&mut self.envs_state, count);
        self.reload_envs();
    }

    fn open_backups(&mut self, env: String) {
        let key = format!("{}.{env}", self.current_site.as_deref().unwrap_or_default());
        let cached = self.cache.backups.get(&key).cloned();
        self.backups_updated = cached.as_ref().map(|c| c.fetched_at);
        self.backups = cached.map(|c| c.items).unwrap_or_default();
        self.backups_state = ListState::default();
        select_first(&mut self.backups_state, self.backups.len());
        self.current_env = Some(env);
        self.screen = Screen::Backups;
        self.reload_backups();
    }
}

// ── free helpers ──────────────────────────────────────────────────────────────

fn err_string(e: anyhow::Error) -> String {
    format!("{e:#}")
}

fn panic_message(e: tokio::task::JoinError) -> String {
    match e.try_into_panic() {
        Ok(payload) => payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "task panicked".into()),
        Err(e) => e.to_string(),
    }
}

/// "terminus env:list my-site --format=json" → "env:list my-site". Flags
/// always come last and may hold quoted spaces, so cut at the first one.
fn short_label(log: &str) -> String {
    log.strip_prefix("terminus ")
        .unwrap_or(log)
        .split_whitespace()
        .take_while(|word| !word.starts_with("--"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn select_first(state: &mut ListState, len: usize) {
    *state = ListState::default();
    if len > 0 {
        state.select(Some(0));
    }
}

/// After a reload, keep the cursor where it was if that row still exists.
fn keep_selection(state: &mut ListState, len: usize) {
    let selected = match (state.selected(), len) {
        (_, 0) => None,
        (Some(i), _) => Some(i.min(len - 1)),
        (None, _) => Some(0),
    };
    state.select(selected);
}

/// After a reload, keep the cursor on the same item (by key) even if rows
/// moved; otherwise keep the position.
fn reselect(state: &mut ListState, keys: &[String], previous: Option<&str>) {
    match previous.and_then(|p| keys.iter().position(|k| k == p)) {
        Some(i) => state.select(Some(i)),
        None => keep_selection(state, keys.len()),
    }
}
