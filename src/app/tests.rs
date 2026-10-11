use super::keys::navigate;
use super::*;
use crate::storage::tests::{temp_config_path, temp_path};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn app() -> App {
    let (tx, _rx) = mpsc::unbounded_channel();
    App::new(
        Config::default(),
        Some(temp_config_path()),
        Some(temp_path("cache.json")),
        tx,
    )
}

fn site(name: &str) -> Site {
    Site {
        name: name.into(),
        plan_name: None,
        framework: None,
        frozen: None,
    }
}

fn env(id: &str) -> Environment {
    Environment {
        id: id.into(),
        domain: None,
        connection_mode: None,
        locked: None,
        initialized: None,
    }
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn type_text(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(key(KeyCode::Char(c)));
    }
}

/// Labels of the tasks the app started (tests never run them).
fn started(app: &App) -> Vec<String> {
    app.running.values().cloned().collect()
}

/// An app on the dashboard with sites loaded and a saved selection.
fn logged_in_app(names: &[&str], selected: Option<&[&str]>) -> App {
    let mut app = app();
    app.screen = Screen::Dashboard;
    app.whoami = Some("me@example.com".into());
    app.config.selected_sites = selected.map(|s| s.iter().map(|n| n.to_string()).collect());
    app.handle_app_event(AppEvent::Sites(Ok(names.iter().map(|n| site(n)).collect())));
    app
}

/// An app on the environments screen of "site" with the given envs.
fn envs_app(ids: &[&str]) -> App {
    let mut app = app();
    app.screen = Screen::Environments;
    app.current_site = Some("site".into());
    app.handle_app_event(AppEvent::Envs {
        site: "site".into(),
        result: Ok(ids.iter().map(|id| env(id)).collect()),
    });
    app
}

// ── sites & selection ─────────────────────────────────────────────────────────

#[test]
fn selection_limits_visible_sites_and_filter_narrows_the_list() {
    let mut app = logged_in_app(&["alpha", "beta", "gamma"], Some(&["alpha", "gamma"]));
    assert_eq!(app.visible_sites().len(), 2);

    app.screen = Screen::Sites;
    app.handle_key(key(KeyCode::Char('/')));
    type_text(&mut app, "GAM");
    let listed: Vec<_> = app.listed_sites().iter().map(|s| s.name.clone()).collect();
    assert_eq!(listed, ["gamma"]);
    // While editing, 'h' is text, not the help shortcut.
    type_text(&mut app, "h");
    assert_eq!(app.overlay, Overlay::None);
    assert_eq!(app.sites_filter.text, "GAMh");

    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.listed_sites().len(), 2);
}

#[test]
fn first_load_without_selection_opens_the_selector() {
    let app = logged_in_app(&["a", "b"], None);
    assert_eq!(app.screen, Screen::SiteSelector);
    assert!(app.selector_first_run);
}

#[test]
fn first_run_selector_cannot_be_cancelled_nor_saved_empty() {
    let mut app = logged_in_app(&["a", "b"], None);
    app.handle_key(key(KeyCode::Char('q')));
    assert_eq!(app.screen, Screen::SiteSelector);
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.screen, Screen::SiteSelector);
    assert!(app.error.is_some());
}

#[test]
fn selector_saves_choice_to_disk() {
    let mut app = logged_in_app(&["a", "b", "c"], None);
    app.handle_key(key(KeyCode::Char('j'))); // cursor on "b"
    app.handle_key(key(KeyCode::Char(' ')));
    app.handle_key(key(KeyCode::Enter));

    assert_eq!(app.screen, Screen::Dashboard);
    assert_eq!(app.config.selected_sites, Some(vec!["b".to_string()]));
    let saved = storage::load(app.config_path.as_ref().unwrap()).unwrap();
    assert_eq!(saved.selected_sites, Some(vec!["b".to_string()]));
    assert_eq!(app.visible_sites().len(), 1);
}

#[test]
fn selector_select_all_none_and_drops_missing_sites() {
    let mut app = logged_in_app(&["a", "b"], Some(&["a", "gone"]));
    app.handle_key(key(KeyCode::Char('S')));
    assert_eq!(app.screen, Screen::SiteSelector);
    assert!(!app.selector_first_run);

    app.handle_key(key(KeyCode::Char('n')));
    assert!(app.selector_checked.is_empty());
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(
        app.config.selected_sites,
        Some(vec!["a".to_string(), "b".to_string()])
    );
}

#[test]
fn selector_cancel_keeps_previous_selection() {
    let mut app = logged_in_app(&["a", "b"], Some(&["a"]));
    app.handle_key(key(KeyCode::Char('S')));
    app.handle_key(key(KeyCode::Char('a')));
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.screen, Screen::Dashboard);
    assert_eq!(app.config.selected_sites, Some(vec!["a".to_string()]));
}

#[test]
fn refresh_keeps_the_cursor_on_the_same_site() {
    let mut app = logged_in_app(&["b", "c"], Some(&["a", "b", "c"]));
    app.screen = Screen::Sites;
    app.handle_key(key(KeyCode::Char('j'))); // on "c"
    app.handle_app_event(AppEvent::Sites(Ok(vec![site("a"), site("b"), site("c")])));
    assert_eq!(app.sites_state.selected(), Some(2));
}

// ── commands ──────────────────────────────────────────────────────────────────

#[test]
fn site_command_with_several_matches_filters_the_list() {
    let mut app = logged_in_app(
        &["shop-a", "shop-b", "blog"],
        Some(&["shop-a", "shop-b", "blog"]),
    );
    app.execute_command("site shop");
    assert_eq!(app.screen, Screen::Sites);
    assert_eq!(app.listed_sites().len(), 2);

    app.execute_command("site nothing");
    assert!(app.error.as_deref().unwrap().contains("No site matches"));
}

#[test]
fn site_command_with_a_unique_match_opens_it() {
    let mut app = logged_in_app(&["shop-a", "blog"], Some(&["shop-a", "blog"]));
    app.execute_command("site blo");
    assert_eq!(app.screen, Screen::Environments);
    assert_eq!(app.current_site.as_deref(), Some("blog"));
    assert_eq!(started(&app).last().unwrap(), "env:list blog");
}

#[test]
fn unknown_and_incomplete_commands_report_errors() {
    let mut app = app();
    app.execute_command("frobnicate");
    assert!(app.error.as_deref().unwrap().starts_with("Unknown command"));
    app.execute_command("site");
    assert_eq!(app.error.as_deref(), Some("Usage: :site <name>"));
    app.execute_command("quit");
    assert!(app.should_quit);
}

#[test]
fn multidev_commands_need_the_environments_screen() {
    let mut app = logged_in_app(&["a"], Some(&["a"]));
    app.screen = Screen::Sites;
    app.execute_command("multidev:create ok");
    assert_eq!(
        app.error.as_deref(),
        Some("Open a site's environments first.")
    );

    let mut app = envs_app(&["dev", "feat"]);
    app.execute_command("multidev:create new-one");
    let Overlay::NewMultidev(form) = &app.overlay else {
        panic!("expected the new multidev dialog");
    };
    assert_eq!(form.name, "new-one");

    app.overlay = Overlay::None;
    app.execute_command("multidev:delete feat");
    assert!(matches!(app.overlay, Overlay::DeleteMultidev(_)));
}

#[test]
fn refresh_command_reloads_the_current_screen() {
    let mut app = envs_app(&["dev"]);
    app.execute_command("refresh");
    assert_eq!(started(&app).last().unwrap(), "env:list site");
}

// ── environments ──────────────────────────────────────────────────────────────

#[test]
fn protected_envs_cannot_be_deleted_and_only_test_live_deploy() {
    let mut app = envs_app(&["dev", "test", "feat"]);

    app.handle_key(key(KeyCode::Char('d')));
    assert!(app.error.as_deref().unwrap().contains("only multidevs"));
    app.handle_key(key(KeyCode::Char('D')));
    assert!(app.error.as_deref().unwrap().contains("Deploys only"));

    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('D')));
    assert!(matches!(
        app.overlay,
        Overlay::Confirm(_, ConfirmAction::Deploy { .. })
    ));
    // Anything but 'y' cancels.
    app.handle_key(key(KeyCode::Char('n')));
    assert_eq!(app.overlay, Overlay::None);
    assert!(!app.is_loading());

    app.handle_key(key(KeyCode::Char('m')));
    assert!(app.error.as_deref().unwrap().contains("Connection mode"));
}

#[test]
fn deploy_runs_only_after_y() {
    let mut app = envs_app(&["dev", "test"]);
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('D')));
    app.handle_key(key(KeyCode::Char('y')));
    assert_eq!(started(&app).last().unwrap(), "env:deploy site.test");
}

#[test]
fn sftp_to_git_asks_for_confirmation() {
    let mut app = app();
    app.screen = Screen::Environments;
    app.current_site = Some("site".into());
    let mut dev = env("dev");
    dev.connection_mode = Some("sftp".into());
    app.handle_app_event(AppEvent::Envs {
        site: "site".into(),
        result: Ok(vec![dev]),
    });
    app.handle_key(key(KeyCode::Char('m')));
    assert!(matches!(
        app.overlay,
        Overlay::Confirm(_, ConfirmAction::SetConnection { mode: "git", .. })
    ));
}

#[test]
fn new_multidev_dialog_validates_and_picks_a_source() {
    let mut app = envs_app(&["dev", "test", "live", "feat"]);
    let mut uninit = env("live");
    uninit.initialized = Some(false);
    app.environments[2] = uninit;

    app.handle_key(key(KeyCode::Char('j'))); // highlight "test"
    app.handle_key(key(KeyCode::Char('n')));
    let form = |app: &App| match &app.overlay {
        Overlay::NewMultidev(form) => form.clone(),
        other => panic!("expected the dialog, got {other:?}"),
    };
    // Uninitialized live can't be a source; the highlighted env is preselected.
    assert_eq!(form(&app).sources, ["dev", "test"]);
    assert_eq!(form(&app).source_env(), "test");

    app.handle_key(key(KeyCode::Tab));
    assert_eq!(form(&app).source_env(), "dev");
    app.handle_key(key(KeyCode::Left));
    assert_eq!(form(&app).source_env(), "test");

    // Invalid and duplicate names block Enter.
    type_text(&mut app, "Bad");
    app.handle_key(key(KeyCode::Enter));
    assert!(form(&app).problem().is_some());
    for _ in 0..3 {
        app.handle_key(key(KeyCode::Backspace));
    }
    type_text(&mut app, "feat");
    app.handle_key(key(KeyCode::Enter));
    assert!(form(&app).problem().unwrap().contains("already exists"));
    assert!(!app.is_loading());

    for _ in 0..4 {
        app.handle_key(key(KeyCode::Backspace));
    }
    type_text(&mut app, "feat-2");
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.overlay, Overlay::None);
    assert_eq!(
        started(&app).last().unwrap(),
        "multidev:create site.test feat-2"
    );
}

#[test]
fn new_multidev_dialog_can_be_cancelled() {
    let mut app = envs_app(&["dev"]);
    app.handle_key(key(KeyCode::Char('n')));
    type_text(&mut app, "abc");
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.overlay, Overlay::None);
    assert!(!app.is_loading());
}

#[test]
fn delete_dialog_requires_typing_the_name() {
    let mut app = envs_app(&["dev", "feat"]);
    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::Char('d')));
    assert!(matches!(app.overlay, Overlay::DeleteMultidev(_)));

    type_text(&mut app, "fea");
    app.handle_key(key(KeyCode::Enter));
    assert!(matches!(app.overlay, Overlay::DeleteMultidev(_)));
    assert!(!app.is_loading());

    type_text(&mut app, "t");
    app.handle_key(key(KeyCode::Tab)); // keep the branch
    let Overlay::DeleteMultidev(form) = &app.overlay else {
        panic!("expected the delete dialog");
    };
    assert!(!form.delete_branch);
    app.handle_key(key(KeyCode::Enter));
    assert_eq!(app.overlay, Overlay::None);
    assert_eq!(started(&app).last().unwrap(), "multidev:delete site.feat");
}

// ── background results, cache & tasks ─────────────────────────────────────────

#[test]
fn stale_environment_results_are_cached_but_not_shown() {
    let mut app = app();
    app.current_site = Some("b".into());
    app.handle_app_event(AppEvent::Envs {
        site: "a".into(),
        result: Ok(vec![env("dev")]),
    });
    assert!(app.environments.is_empty());
    assert!(app.cache.envs.contains_key("a"));
}

#[test]
fn opening_a_site_shows_cached_envs_while_refreshing() {
    let mut app = logged_in_app(&["a"], Some(&["a"]));
    app.cache.envs.insert(
        "a".into(),
        Cached {
            fetched_at: 42,
            items: vec![env("dev"), env("feat")],
        },
    );
    app.execute_command("site a");
    assert_eq!(app.environments.len(), 2);
    assert_eq!(app.envs_updated, Some(42));
    assert_eq!(app.data_updated(), Some(42));
    assert!(app.is_loading());
}

#[test]
fn results_are_persisted_to_the_cache_file() {
    let mut app = logged_in_app(&["a"], Some(&["a"]));
    let saved = storage::load_cache(app.cache_path.as_ref().unwrap());
    assert_eq!(saved.sites.unwrap().items, vec![site("a")]);

    app.current_site = Some("a".into());
    app.current_env = Some("dev".into());
    app.handle_app_event(AppEvent::Backups {
        site: "a".into(),
        env: "dev".into(),
        result: Ok(vec![]),
    });
    let saved = storage::load_cache(app.cache_path.as_ref().unwrap());
    assert!(saved.backups.contains_key("a.dev"));
}

#[test]
fn removed_sites_are_pruned_from_the_cache() {
    let mut app = logged_in_app(&["a", "b"], Some(&["a", "b"]));
    for name in ["a", "b"] {
        app.cache.envs.insert(
            name.into(),
            Cached {
                fetched_at: 1,
                items: vec![],
            },
        );
    }
    app.cache.backups.insert(
        "b.dev".into(),
        Cached {
            fetched_at: 1,
            items: vec![],
        },
    );
    app.handle_app_event(AppEvent::Sites(Ok(vec![site("a")])));
    assert!(app.cache.envs.contains_key("a"));
    assert!(!app.cache.envs.contains_key("b"));
    assert!(app.cache.backups.is_empty());
}

#[test]
fn cache_of_another_account_is_discarded() {
    let mut app = logged_in_app(&["a"], Some(&["a"]));
    app.cache.user = Some("old@example.com".into());
    app.adopt_user("new@example.com");
    assert!(app.sites.is_empty());
    assert!(!app.sites_loaded);
    assert_eq!(app.cache.sites, None);
    assert_eq!(app.cache.user.as_deref(), Some("new@example.com"));
}

#[test]
fn expired_session_after_optimistic_start_goes_to_login() {
    let mut app = logged_in_app(&["a"], Some(&["a"]));
    app.handle_app_event(AppEvent::Session(Err("not logged in".into())));
    assert_eq!(app.screen, Screen::Login);
    assert!(app.error.as_deref().unwrap().contains("expired"));
    assert!(app.sites.is_empty());
}

#[test]
fn running_tasks_drive_the_activity_indicator() {
    let mut app = envs_app(&["dev"]);
    app.handle_key(key(KeyCode::Char('r')));
    app.handle_key(key(KeyCode::Char('w')));
    assert_eq!(app.activity(), Some(("env:wake site.dev", 1)));

    let ids: Vec<u64> = app.running.keys().copied().collect();
    app.handle_task_done(TaskDone {
        id: ids[1],
        event: AppEvent::Op {
            what: "Wake".into(),
            result: Err("boom".into()),
            refresh: Refresh::Nothing,
        },
    });
    assert_eq!(app.activity(), Some(("env:list site", 0)));
    assert_eq!(app.error.as_deref(), Some("Wake failed: boom"));
}

#[test]
fn short_labels_drop_the_program_and_flags() {
    assert_eq!(
        short_label("terminus env:list my-site --format=json"),
        "env:list my-site"
    );
    assert_eq!(short_label("open https://x"), "open https://x");
    assert_eq!(
        short_label("terminus env:deploy s.test --note=\"Deployed from sunim\""),
        "env:deploy s.test"
    );
}

// ── keys & overlays ───────────────────────────────────────────────────────────

#[test]
fn esc_dismisses_message_first_then_goes_back() {
    let mut app = logged_in_app(&["a"], Some(&["a"]));
    app.screen = Screen::Sites;
    app.error = Some("oops".into());
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.screen, Screen::Sites);
    assert!(app.error.is_none());
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.screen, Screen::Dashboard);
}

#[test]
fn help_scrolls_within_bounds_and_closes() {
    let mut app = logged_in_app(&["a"], Some(&["a"]));
    app.handle_key(key(KeyCode::Char('?')));
    assert_eq!(app.overlay, Overlay::Help);
    app.help_max_scroll.set(20);

    app.handle_key(key(KeyCode::Char('j')));
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.help_scroll, 11);
    app.handle_key(key(KeyCode::Char('G')));
    assert_eq!(app.help_scroll, 20);
    app.handle_key(key(KeyCode::PageDown));
    assert_eq!(app.help_scroll, 20);
    app.handle_key(key(KeyCode::Char('g')));
    assert_eq!(app.help_scroll, 0);
    // Unrelated keys don't close it.
    app.handle_key(key(KeyCode::Char('x')));
    assert_eq!(app.overlay, Overlay::Help);
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.overlay, Overlay::None);
}

#[test]
fn success_messages_expire_but_errors_stay() {
    let mut app = app();
    app.set_status("done");
    app.tick();
    assert_eq!(app.status.as_deref(), Some("done"));
    app.status_set_at = Some(Instant::now() - STATUS_TTL);
    app.tick();
    assert_eq!(app.status, None);

    app.set_error("bad");
    app.tick();
    assert_eq!(app.error.as_deref(), Some("bad"));
}

#[test]
fn log_is_capped_and_scroll_stays_anchored() {
    let mut app = app();
    for i in 0..MAX_LOG_ENTRIES + 10 {
        app.push_log(LogLevel::Info, format!("{i}"));
    }
    assert_eq!(app.log_entries.len(), MAX_LOG_ENTRIES);
    assert_eq!(app.log_entries[0].message, "10");

    app.log_scroll = 3;
    app.push_log(LogLevel::Info, "new");
    assert_eq!(app.log_scroll, 4);
}

#[test]
fn ctrl_chars_are_not_typed_into_inputs() {
    let mut app = app();
    app.handle_key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL));
    app.handle_key(key(KeyCode::Char('x')));
    assert_eq!(app.token_input, "x");
}

#[test]
fn navigation_is_clamped() {
    let mut state = ListState::default();
    assert!(navigate(&mut state, 3, KeyCode::Char('G')));
    assert_eq!(state.selected(), Some(2));
    navigate(&mut state, 3, KeyCode::Down);
    assert_eq!(state.selected(), Some(2));
    navigate(&mut state, 3, KeyCode::Home);
    navigate(&mut state, 3, KeyCode::Up);
    assert_eq!(state.selected(), Some(0));
    assert!(navigate(&mut state, 0, KeyCode::Down));
    assert_eq!(state.selected(), None);
    assert!(!navigate(&mut state, 3, KeyCode::Char('x')));
}

#[test]
fn environments_filter_narrows_list_and_esc_clears_it() {
    let mut app = envs_app(&["dev", "test", "feat-auth", "feat-ui"]);
    assert_eq!(app.listed_environments().len(), 4);

    app.handle_key(key(KeyCode::Char('/')));
    type_text(&mut app, "auth");
    let listed: Vec<_> = app
        .listed_environments()
        .iter()
        .map(|e| e.id.clone())
        .collect();
    assert_eq!(listed, ["feat-auth"]);

    // While editing, 'h' is typed into the filter, not help overlay
    type_text(&mut app, "h");
    assert_eq!(app.overlay, Overlay::None);
    assert_eq!(app.envs_filter.text, "authh");

    // Esc clears filter
    app.handle_key(key(KeyCode::Esc));
    assert_eq!(app.listed_environments().len(), 4);
}

#[test]
fn copy_url_and_backup_file_to_clipboard() {
    let mut app = envs_app(&["dev"]);
    let mut dev = env("dev");
    dev.domain = Some("dev-site.pantheonsite.io".into());
    app.environments[0] = dev;

    app.handle_key(key(KeyCode::Char('y')));
    assert!(
        app.status
            .as_deref()
            .unwrap()
            .contains("dev-site.pantheonsite.io")
    );

    // Backups screen copy
    app.screen = Screen::Backups;
    app.backups = vec![Backup {
        file: "site_dev_123_code.tar.gz".into(),
        size: Some("10MB".into()),
        date: Some(1234.0),
        expiry: None,
        initiator: None,
    }];
    app.backups_state.select(Some(0));
    app.handle_key(key(KeyCode::Char('y')));
    assert!(
        app.status
            .as_deref()
            .unwrap()
            .contains("site_dev_123_code.tar.gz")
    );
}
