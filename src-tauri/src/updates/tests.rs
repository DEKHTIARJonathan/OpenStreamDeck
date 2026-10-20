use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tauri_plugin_updater::Updater;

fn app() -> tauri::App<tauri::test::MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context
        .config_mut()
        .plugins
        .0
        .insert("updater".into(), serde_json::json!({"pubkey": ""}));
    tauri::test::mock_builder()
        .plugin(
            tauri_plugin_updater::Builder::new()
                .pubkey(test_feed::fixture().public_key)
                .build(),
        )
        .build(context)
        .unwrap()
}
fn service() -> Service {
    Service::new("0.1.0".into(), None, |_| {})
}
async fn updater(
    app: &tauri::App<tauri::test::MockRuntime>,
    scenario: &str,
) -> (test_feed::Feed, Updater) {
    let feed = test_feed::Feed::serve(scenario).await.unwrap();
    let updater = test_feed::builder(app.handle(), &feed)
        .unwrap()
        .build()
        .unwrap();
    (feed, updater)
}
async fn check(service: &Service, updater: &Updater) -> Status {
    service
        .check(async { updater.check().await.map_err(update_error) })
        .await
        .unwrap()
}

fn verified_install(
    update: Update,
    bytes: Vec<u8>,
) -> std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>> {
    Box::pin(async move {
        update.verify(&bytes).unwrap();
        Ok(())
    })
}
async fn apply(service: &Service) -> Status {
    service
        .apply(false, || Ok(()), || {}, verified_install, || Ok(()))
        .await
        .unwrap()
}

#[tokio::test]
async fn one_click_downloads_verifies_installs_and_restarts_in_order() {
    let app = app();
    let (_feed, updater) = updater(&app, "available").await;
    let events = Arc::new(Mutex::new(Vec::new()));
    let recorded = events.clone();
    let service = Service::new("0.0.1".into(), None, move |status| {
        recorded.lock().unwrap().push(status.clone())
    });
    assert_eq!(check(&service, &updater).await.phase, Phase::Available);
    let calls = Mutex::new(Vec::new());
    let installed = service
        .apply(
            false,
            || {
                calls.lock().unwrap().push("acquire");
                Ok(())
            },
            || panic!("successful update must retain the work gate until restart"),
            |update, bytes| {
                assert_eq!(service.status().phase, Phase::Installing);
                update.verify(&bytes).unwrap();
                calls.lock().unwrap().push("install");
                Box::pin(async { Ok(()) })
            },
            || {
                assert_eq!(service.status().phase, Phase::Installed);
                calls.lock().unwrap().push("restart");
                Ok(())
            },
        )
        .await
        .unwrap();
    assert_eq!(installed.phase, Phase::Installed);
    assert_eq!(*calls.lock().unwrap(), ["acquire", "install", "restart"]);
    assert_eq!(
        installed.downloaded,
        (test_feed::fixture().seed.len() * test_feed::fixture().repeat) as u64
    );
    assert_eq!(installed.total, Some(installed.downloaded));
    let events = events.lock().unwrap();
    assert!(events.iter().any(|s| s.phase == Phase::Downloading
        && s.downloaded > 0
        && s.downloaded < s.total.unwrap()));
    assert!(events
        .windows(2)
        .all(|pair| pair[0].revision < pair[1].revision));
}

#[tokio::test]
async fn feed_failures_and_no_update_cannot_start_installation() {
    let app = app();
    for (scenario, expected) in [
        ("current", Phase::Current),
        ("malformed", Phase::Error),
        ("platform", Phase::Error),
        ("http", Phase::Error),
    ] {
        let (_feed, updater) = updater(&app, scenario).await;
        let service = service();
        assert_eq!(check(&service, &updater).await.phase, expected);
        assert!(service
            .apply(
                false,
                || panic!("no update"),
                || panic!("not acquired"),
                verified_install,
                || panic!("not installed")
            )
            .await
            .is_err());
    }
}

#[tokio::test]
async fn failed_downloads_release_the_gate_without_installing_and_allow_retry() {
    let app = app();
    for scenario in ["corrupt", "interrupted"] {
        let (_feed, updater) = updater(&app, scenario).await;
        let service = service();
        check(&service, &updater).await;
        let released = AtomicUsize::new(0);
        let status = service
            .apply(
                false,
                || Ok(()),
                || {
                    released.fetch_add(1, Ordering::SeqCst);
                },
                |_, _| panic!("unverified payload"),
                || panic!("not installed"),
            )
            .await
            .unwrap();
        assert_eq!(status.phase, Phase::Error);
        assert_eq!(released.load(Ordering::SeqCst), 1);
        assert!(!status.applying());
        if scenario == "corrupt" {
            assert!(status
                .error
                .unwrap()
                .contains("signature verification failed"));
        }
        let (_good_feed, good) = super::tests::updater(&app, "available").await;
        check(&service, &good).await;
        assert_eq!(apply(&service).await.phase, Phase::Installed);
    }
}

#[tokio::test]
async fn dirty_unsupported_and_active_workflows_are_rejected_before_downloading() {
    let app = app();
    let (_feed, updater) = updater(&app, "available").await;
    let service = service();
    check(&service, &updater).await;
    assert!(service
        .apply(
            true,
            || panic!("dirty"),
            || panic!("not acquired"),
            verified_install,
            || panic!("not installed")
        )
        .await
        .unwrap_err()
        .contains("Save"));
    service.inner.lock().unwrap().status.install_disabled = Some("Unsupported installation".into());
    assert!(service
        .apply(
            false,
            || panic!("unsupported"),
            || panic!("not acquired"),
            verified_install,
            || panic!("not installed")
        )
        .await
        .unwrap_err()
        .contains("Unsupported"));
    service.inner.lock().unwrap().status.install_disabled = None;
    assert!(service
        .apply(
            false,
            || Err("Active workflow".into()),
            || panic!("not acquired"),
            verified_install,
            || panic!("not installed")
        )
        .await
        .unwrap_err()
        .contains("Active workflow"));
    assert_eq!(service.status().phase, Phase::Available);
    assert_eq!(service.status().downloaded, 0);
}

#[tokio::test]
async fn installation_failure_retries_and_restart_failure_never_reinstalls() {
    let app = app();
    let (_feed, updater) = updater(&app, "available").await;
    let service = service();
    check(&service, &updater).await;
    let released = AtomicUsize::new(0);
    let failed = service
        .apply(
            false,
            || Ok(()),
            || {
                released.fetch_add(1, Ordering::SeqCst);
            },
            |update, bytes| {
                update.verify(&bytes).unwrap();
                Box::pin(async { Err("Permission denied".into()) })
            },
            || panic!("failed installation must not restart"),
        )
        .await
        .unwrap();
    assert_eq!(failed.phase, Phase::Error);
    assert!(failed.error.unwrap().contains("Permission denied"));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    let failed = service
        .apply(
            false,
            || Ok(()),
            || {
                released.fetch_add(1, Ordering::SeqCst);
            },
            verified_install,
            || Err("OS restart failure".into()),
        )
        .await
        .unwrap();
    assert_eq!(failed.phase, Phase::Installed);
    assert!(failed.error.as_ref().unwrap().contains("Restart failed"));
    assert!(!failed.applying());
    assert_eq!(released.load(Ordering::SeqCst), 2);
    assert!(service.check(async { Ok(None) }).await.is_err());
    let retries = AtomicUsize::new(0);
    let result = service
        .apply(
            false,
            || Ok(()),
            || panic!("successful restart"),
            |_, _| panic!("restart retry must not reinstall"),
            || {
                retries.fetch_add(1, Ordering::SeqCst);
                Ok(())
            },
        )
        .await
        .unwrap();
    assert!(result.error.is_none());
    assert_eq!(retries.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn download_and_install_are_one_serialized_uninterrupted_operation() {
    let app = app();
    let (_feed, updater) = updater(&app, "available").await;
    let service = service();
    check(&service, &updater).await;
    let (entered, entering) = tokio::sync::oneshot::channel();
    let (finish, wait) = tokio::sync::oneshot::channel();
    let operation = service.apply(
        false,
        || Ok(()),
        || panic!("successful update"),
        |update, bytes| {
            Box::pin(async move {
                update.verify(&bytes).unwrap();
                entered.send(()).unwrap();
                wait.await.unwrap();
                Ok(())
            })
        },
        || Ok(()),
    );
    tokio::pin!(operation);
    tokio::select! { _ = &mut operation => panic!("download should take time"), _ = tokio::time::sleep(Duration::from_millis(80)) => {} }
    assert_eq!(service.status().phase, Phase::Downloading);
    assert!(service.status().blocks_exit(None));
    assert!(service.check(async { Ok(None) }).await.is_err());
    assert!(service
        .apply(
            false,
            || panic!("duplicate"),
            || panic!("not acquired"),
            verified_install,
            || Ok(())
        )
        .await
        .is_err());
    tokio::select! { _ = &mut operation => panic!("installer should wait"), _ = entering => {} }
    assert_eq!(service.status().phase, Phase::Installing);
    assert!(service.status().blocks_exit(Some(0)));
    assert!(service
        .apply(
            false,
            || panic!("duplicate"),
            || panic!("not acquired"),
            verified_install,
            || Ok(())
        )
        .await
        .is_err());
    finish.send(()).unwrap();
    assert_eq!(operation.await.unwrap().phase, Phase::Installed);
    assert!(service.status().blocks_exit(Some(0)));
    assert!(!service.status().blocks_exit(Some(tauri::RESTART_EXIT_CODE)));
}

#[tokio::test]
async fn timeouts_release_the_gate_and_remain_retryable() {
    let app = app();
    let (_feed, updater) = updater(&app, "available").await;
    let service = service();
    check(&service, &updater).await;
    service
        .inner
        .lock()
        .unwrap()
        .update
        .as_mut()
        .unwrap()
        .timeout = Some(Duration::from_millis(50));
    let released = AtomicUsize::new(0);
    let failed = service
        .apply(
            false,
            || Ok(()),
            || {
                released.fetch_add(1, Ordering::SeqCst);
            },
            |_, _| panic!("incomplete download"),
            || panic!("not installed"),
        )
        .await
        .unwrap();
    assert_eq!(failed.phase, Phase::Error);
    assert!(failed.error.unwrap().contains("timed out"));
    assert_eq!(released.load(Ordering::SeqCst), 1);
    check(&service, &updater).await;
    assert_eq!(apply(&service).await.phase, Phase::Installed);
    assert!(
        Service::deadline::<()>(Duration::from_millis(10), std::future::pending())
            .await
            .unwrap_err()
            .contains("timed out")
    );
}

#[test]
fn only_the_automatic_restart_can_exit_an_active_update() {
    let mut status = service().status();
    for phase in [Phase::Downloading, Phase::Installing, Phase::Installed] {
        status.phase = phase;
        assert!(status.applying());
        assert!(status.blocks_exit(None));
        assert!(status.blocks_exit(Some(0)));
        assert_eq!(
            status.blocks_exit(Some(tauri::RESTART_EXIT_CODE)),
            phase != Phase::Installed
        );
    }
    status.error = Some("Restart failed".into());
    assert!(!status.applying());
    assert!(!status.blocks_exit(None));
}

#[tokio::test]
async fn wrong_key_is_rejected_by_real_plugin_verification() {
    let app = app();
    let feed = test_feed::Feed::start().await.unwrap();
    use base64::Engine;
    let fixture = test_feed::fixture();
    let decoded = String::from_utf8(
        base64::engine::general_purpose::STANDARD
            .decode(&fixture.public_key)
            .unwrap(),
    )
    .unwrap();
    let mut lines = decoded.lines();
    let comment = lines.next().unwrap();
    let mut key = base64::engine::general_purpose::STANDARD
        .decode(lines.next().unwrap())
        .unwrap();
    key[15] ^= 1;
    let wrong = base64::engine::general_purpose::STANDARD.encode(format!(
        "{comment}\n{}",
        base64::engine::general_purpose::STANDARD.encode(key)
    ));
    let updater = test_feed::builder(app.handle(), &feed)
        .unwrap()
        .pubkey(wrong)
        .build()
        .unwrap();
    let service = service();
    check(&service, &updater).await;
    let status = apply(&service).await;
    assert_eq!(status.phase, Phase::Error);
    assert!(status
        .error
        .unwrap()
        .contains("signature verification failed"));
}

#[test]
fn native_install_gate_refuses_active_actions_and_blocks_new_actions() {
    let (tx, _rx) = std::sync::mpsc::sync_channel(4);
    let state = crate::AppState::new(
        std::path::PathBuf::new(),
        crate::model::Config::default(),
        tx,
    );
    state
        .active
        .lock()
        .unwrap()
        .insert("lcd1".into(), "running".into());
    assert!(state.begin_update_install().is_err());
    assert!(!state.update_installing.load(Ordering::SeqCst));
    state.active.lock().unwrap().clear();
    state.begin_update_install().unwrap();
    assert!(state.update_installing.load(Ordering::SeqCst));
}

#[test]
fn native_install_gate_waits_for_overlapping_runs_after_latest_binding_completes() {
    let (tx, _rx) = std::sync::mpsc::sync_channel(4);
    let state = crate::AppState::new(
        std::path::PathBuf::new(),
        crate::model::Config::default(),
        tx,
    );
    // Repeatable actions replace the binding's latest run, but both futures
    // retain a cancellation entry until completion. The newest can finish first.
    state
        .active
        .lock()
        .unwrap()
        .insert("lcd1".into(), "newer".into());
    for id in ["older", "newer"] {
        state.cancellations.lock().unwrap().insert(
            id.into(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
    }
    state.active.lock().unwrap().remove("lcd1");
    state.cancellations.lock().unwrap().remove("newer");
    assert!(state.begin_update_install().is_err());
    assert!(!state.update_installing.load(Ordering::SeqCst));
    state.cancellations.lock().unwrap().remove("older");
    state.begin_update_install().unwrap();
    assert!(state.update_installing.load(Ordering::SeqCst));
}

#[test]
fn native_save_rejects_installation_before_touching_login_or_configuration() {
    struct Login(std::cell::Cell<usize>);
    impl crate::settings::LoginRegistration for Login {
        fn is_enabled(&self) -> Result<bool, String> {
            self.0.set(self.0.get() + 1);
            Ok(false)
        }
        fn set_enabled(&self, _: bool) -> Result<(), String> {
            panic!("unchanged login preference must not be written")
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let (tx, _rx) = std::sync::mpsc::sync_channel(4);
    let initial = crate::model::Config::default();
    crate::store::save(dir.path(), &initial).unwrap();
    let saved = std::fs::read(dir.path().join("config.json")).unwrap();
    let state = crate::AppState::new(dir.path().to_owned(), initial.clone(), tx);
    let login = Login(std::cell::Cell::new(0));
    let next = crate::model::Config {
        brightness: 42,
        ..initial
    };
    state.begin_update_install().unwrap();
    assert!(state
        .save_configuration(next.clone(), &login)
        .unwrap_err()
        .contains("update is in progress"));
    assert_eq!(login.0.get(), 0);
    assert_eq!(
        std::fs::read(dir.path().join("config.json")).unwrap(),
        saved
    );
    assert_ne!(state.config.lock().unwrap().brightness, 42);

    // Failure recovery releases the gate and restores ordinary persistence.
    state.update_installing.store(false, Ordering::SeqCst);
    assert!(state.save_configuration(next, &login).unwrap());
    assert_eq!(login.0.get(), 1);
    assert_eq!(crate::store::load(dir.path()).unwrap().brightness, 42);
    assert_eq!(
        std::fs::read(dir.path().join("config.backup.json")).unwrap(),
        saved
    );
}

#[test]
fn native_page_selection_respects_installation_gate_and_recovers() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, _rx) = std::sync::mpsc::sync_channel(4);
    let mut initial = crate::model::Config::default();
    let mut second = initial.pages[0].clone();
    second.id = "second".into();
    initial.pages.push(second);
    crate::store::save(dir.path(), &initial).unwrap();
    let saved = std::fs::read(dir.path().join("config.json")).unwrap();
    let state = crate::AppState::new(dir.path().to_owned(), initial, tx);
    state.begin_update_install().unwrap();
    assert!(crate::page_context::select_state(&state, "second")
        .unwrap_err()
        .contains("update is in progress"));
    assert_eq!(
        std::fs::read(dir.path().join("config.json")).unwrap(),
        saved
    );
    assert!(!dir.path().join("config.backup.json").exists());
    assert_eq!(state.config.lock().unwrap().active_page, "home");
    assert_eq!(state.page_context.lock().unwrap().desired_page, "home");
    state.update_installing.store(false, Ordering::SeqCst);
    crate::page_context::select_state(&state, "second").unwrap();
    assert_eq!(
        crate::store::load(dir.path()).unwrap().active_page,
        "second"
    );
    assert_eq!(state.page_context.lock().unwrap().desired_page, "second");
    assert_eq!(
        std::fs::read(dir.path().join("config.backup.json")).unwrap(),
        saved
    );
}

#[tokio::test]
async fn production_transport_rejects_plain_http() {
    let app = app();
    let feed = test_feed::Feed::start().await.unwrap();
    let updater = transport_builder(app.handle(), false)
        .target("test-target")
        .no_proxy()
        .endpoints(vec![feed.url.parse().unwrap()])
        .unwrap()
        .build()
        .unwrap();
    assert!(updater.check().await.is_err());
}

#[tokio::test]
async fn development_transport_checks_localhost_and_verifies_download() {
    let app = app();
    let feed = test_feed::Feed::start().await.unwrap();
    let updater = transport_builder(app.handle(), true)
        .target("test-target")
        .no_proxy()
        .endpoints(vec![feed.url.parse().unwrap()])
        .unwrap()
        .build()
        .unwrap();
    let service = Service::new(
        "0.0.1".into(),
        installation_block_reason(
            "macos",
            Some(std::path::Path::new(
                "/checkout/target/debug/OpenStreamDeck.app/Contents/MacOS/openstreamdeck",
            )),
            false,
        ),
        |_| {},
    );
    assert_eq!(check(&service, &updater).await.phase, Phase::Available);
    assert!(service.status().install_disabled.is_none());
    assert_eq!(apply(&service).await.phase, Phase::Installed);
}

#[test]
fn installation_policy_accepts_development_app_bundles_and_rejects_unbundled_macos() {
    use std::path::Path;
    for executable in [
        "/checkout/target/debug/OpenStreamDeck.app/Contents/MacOS/openstreamdeck",
        "/Applications/OpenStreamDeck.app/Contents/MacOS/openstreamdeck",
    ] {
        assert!(installation_block_reason("macos", Some(Path::new(executable)), false).is_none());
    }
    for executable in [
        "/checkout/target/debug/openstreamdeck",
        "/checkout/Contents/openstreamdeck",
        "/checkout/Contents/MacOS/openstreamdeck",
        "/checkout/OpenStreamDeck.app/openstreamdeck",
    ] {
        assert!(installation_block_reason("macos", Some(Path::new(executable)), false).is_some());
    }
    assert!(installation_block_reason("macos", None, false).is_some());
    assert!(installation_block_reason("linux", None, false).is_some());
    assert!(installation_block_reason("linux", None, true).is_none());
    assert!(installation_block_reason("windows", None, false).is_none());
}

#[test]
fn http_payloads_and_redirects_are_limited_to_development_localhost() {
    for value in [
        "http://localhost:8080/payload",
        "http://127.0.0.1:8080/payload",
        "http://[::1]:8080/payload",
    ] {
        let url = value.parse().unwrap();
        assert!(transport_allowed(&url, true));
        assert!(!transport_allowed(&url, false));
    }
    for value in [
        "http://example.com/payload",
        "http://localhost.example.com/payload",
        "file:///tmp/payload",
    ] {
        let url = value.parse().unwrap();
        assert!(!transport_allowed(&url, true));
        assert!(!transport_allowed(&url, false));
    }
    let url = "https://github.com/owner/repository/releases/download/v0.2.0/payload"
        .parse()
        .unwrap();
    assert!(transport_allowed(&url, true));
    assert!(transport_allowed(&url, false));
}

#[tokio::test(start_paused = true)]
async fn scheduler_checks_on_startup_then_six_hours_after_the_last_manual_or_auto_check() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("updater-state.json");
    history::save(
        &path,
        &history::History {
            last_checked_at: Some(history::now_ms()),
            skipped_version: None,
        },
    )
    .unwrap();
    let service = Arc::new(service().with_history(path.clone()));
    assert!(service.status().last_checked_at.is_some());
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let task = tokio::spawn(run_scheduler(service.clone(), move || {
        count.fetch_add(1, Ordering::SeqCst);
        async { Ok(None) }
    }));
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1, "startup always checks");
    assert_eq!(service.status().phase, Phase::Current);
    assert_eq!(
        history::load(&path).unwrap().last_checked_at,
        service.status().last_checked_at
    );
    tokio::time::advance(CHECK_INTERVAL - Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    tokio::time::advance(Duration::from_secs(3 * 3600)).await;
    service.check(async { Ok(None) }).await.unwrap();
    tokio::time::advance(Duration::from_secs(3 * 3600)).await;
    tokio::task::yield_now().await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "manual check resets the six-hour delay"
    );
    tokio::time::advance(Duration::from_secs(3 * 3600)).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 3);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[tokio::test(start_paused = true)]
async fn scheduled_checks_defer_during_installation_and_resume_without_a_burst() {
    let service = Arc::new(service());
    let calls = Arc::new(AtomicUsize::new(0));
    let count = calls.clone();
    let task = tokio::spawn(run_scheduler(service.clone(), move || {
        count.fetch_add(1, Ordering::SeqCst);
        async { Ok(None) }
    }));
    tokio::task::yield_now().await;
    for phase in [
        Phase::Checking,
        Phase::Downloading,
        Phase::Installing,
        Phase::Installed,
    ] {
        service.inner.lock().unwrap().status.phase = phase;
        tokio::time::advance(CHECK_INTERVAL).await;
        tokio::task::yield_now().await;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    service.inner.lock().unwrap().status.phase = Phase::Error;
    tokio::time::advance(SCHEDULE_POLL).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    tokio::time::advance(SCHEDULE_POLL).await;
    tokio::task::yield_now().await;
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
}

#[tokio::test(start_paused = true)]
async fn scheduler_handles_sleep_and_clock_rollback_and_records_failed_attempts() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("updater-state.json");
    let service = service().with_history(path.clone());
    let status = service
        .check(async { Err("Offline".into()) })
        .await
        .unwrap();
    assert_eq!(status.phase, Phase::Error);
    assert!(status.last_checked_at.is_some());
    assert_eq!(
        history::load(&path).unwrap().last_checked_at,
        status.last_checked_at
    );
    assert_eq!(service.automatic_delay(), SCHEDULE_POLL);
    service.inner.lock().unwrap().history.last_checked_at =
        Some(history::now_ms() - CHECK_INTERVAL.as_millis() as u64);
    assert_eq!(
        service.automatic_delay(),
        Duration::ZERO,
        "wall time catches up after sleep"
    );
    service.check(async { Ok(None) }).await.unwrap();
    service.inner.lock().unwrap().history.last_checked_at =
        Some(history::now_ms() + 24 * 3600 * 1000);
    tokio::time::advance(CHECK_INTERVAL).await;
    assert_eq!(
        service.automatic_delay(),
        Duration::ZERO,
        "clock rollback cannot delay the monotonic deadline"
    );
}

#[tokio::test]
async fn automatic_reminders_persist_skips_and_reenable_only_for_a_different_version() {
    let app = app();
    let (_feed, updater) = updater(&app, "available").await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("updater-state.json");
    let foregrounds = Arc::new(AtomicUsize::new(0));
    let events = Arc::new(Mutex::new(Vec::<Status>::new()));
    let recorded = events.clone();
    let mut service = Service::new("0.0.1".into(), None, move |status| {
        recorded.lock().unwrap().push(status.clone())
    })
    .with_history(path.clone());
    let calls = foregrounds.clone();
    service.foreground = Box::new(move || {
        assert_eq!(
            events
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .prompt_version
                .as_deref(),
            Some("9999.0.0"),
            "notice must be stored before creating a webview"
        );
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let found = service
        .check_inner(async { updater.check().await.map_err(update_error) }, true)
        .await
        .unwrap();
    assert_eq!(found.prompt_version.as_deref(), Some("9999.0.0"));
    assert_eq!(foregrounds.load(Ordering::SeqCst), 1);
    assert!(service.skip("older-version").is_err());
    assert!(service.skip("9999.0.0").unwrap().prompt_version.is_none());
    assert_eq!(
        history::load(&path).unwrap().skipped_version.as_deref(),
        Some("9999.0.0")
    );
    // Restart with the same metadata, as a background/tray launch would.
    let mut restored = Service::new("0.0.1".into(), None, |_| {}).with_history(path.clone());
    let calls = foregrounds.clone();
    restored.foreground = Box::new(move || {
        calls.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    assert_eq!(restored.automatic_delay(), Duration::ZERO);
    let same = restored
        .check_inner(async { updater.check().await.map_err(update_error) }, true)
        .await
        .unwrap();
    assert_eq!(same.phase, Phase::Available);
    assert!(same.prompt_version.is_none());
    assert_eq!(
        foregrounds.load(Ordering::SeqCst),
        1,
        "skipped releases do not foreground or prompt"
    );
    // Manual checks still offer installation of a skipped release.
    assert_eq!(check(&restored, &updater).await.phase, Phase::Available);
    assert_eq!(
        history::load(&path).unwrap().skipped_version.as_deref(),
        Some("9999.0.0")
    );
    let mut newer = updater.check().await.unwrap().unwrap();
    newer.version = "10000.0.0".into();
    let found = restored
        .check_inner(async { Ok(Some(newer)) }, true)
        .await
        .unwrap();
    assert_eq!(found.prompt_version.as_deref(), Some("10000.0.0"));
    assert_eq!(foregrounds.load(Ordering::SeqCst), 2);
    assert_eq!(apply(&restored).await.phase, Phase::Installed);
    assert!(restored.status().prompt_version.is_none());
}

#[tokio::test]
async fn reminder_foreground_and_persistence_failures_remain_visible_and_retryable() {
    let app = app();
    let (_feed, updater) = updater(&app, "available").await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("updater-state.json");
    let mut service = service().with_history(path.clone());
    service.foreground = Box::new(|| Err("OS focus failure".into()));
    let status = service
        .check_inner(async { updater.check().await.map_err(update_error) }, true)
        .await
        .unwrap();
    assert_eq!(status.phase, Phase::Available);
    assert!(status.error.unwrap().contains("OS focus failure"));
    assert!(status.prompt_version.is_some());
    let saved = std::fs::read(&path).unwrap();
    std::fs::create_dir(path.with_extension("tmp")).unwrap();
    assert!(service
        .skip("9999.0.0")
        .unwrap_err()
        .contains("save update history"));
    assert!(service.status().prompt_version.is_some());
    assert!(service.status().history_error.is_some());
    assert_eq!(std::fs::read(&path).unwrap(), saved);
    assert!(service
        .inner
        .lock()
        .unwrap()
        .history
        .skipped_version
        .is_none());
    std::fs::remove_dir(path.with_extension("tmp")).unwrap();
    service.skip("9999.0.0").unwrap();
    assert!(service.status().history_error.is_none());
}

#[tokio::test]
async fn history_errors_do_not_block_checks() {
    let app = app();
    let (_feed, updater) = updater(&app, "available").await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("updater-state.json");
    std::fs::write(&path, b"malformed history").unwrap();
    let service = service().with_history(path.clone());
    assert!(service
        .status()
        .history_error
        .unwrap()
        .contains("malformed"));
    assert_eq!(std::fs::read(&path).unwrap(), b"malformed history");
    let status = service
        .check_inner(async { updater.check().await.map_err(update_error) }, true)
        .await
        .unwrap();
    assert!(status.history_error.is_none());
    assert!(status.last_checked_at.is_some());
    assert!(status.prompt_version.is_some());
    service.skip("9999.0.0").unwrap();
    // Saving a failed check must preserve the last successfully written history.
    let saved = std::fs::read(&path).unwrap();
    std::fs::create_dir(path.with_extension("tmp")).unwrap();
    let status = service
        .check(async { Err("Network unavailable".into()) })
        .await
        .unwrap();
    assert!(status.history_error.is_some());
    assert!(status.last_checked_at.is_some());
    assert!(status.error.unwrap().contains("Network unavailable"));
    assert_eq!(std::fs::read(&path).unwrap(), saved);
}
