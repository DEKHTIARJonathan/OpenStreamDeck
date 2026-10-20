//! Application preferences. Only the OS boundary is replaced in tests.
use crate::{model::Config, store};
use std::path::Path;
use tauri_plugin_autostart::ManagerExt;

pub(crate) trait LoginRegistration {
    fn is_enabled(&self) -> Result<bool, String>;
    fn set_enabled(&self, enabled: bool) -> Result<(), String>;
}

pub(crate) struct NativeLogin<'a>(pub &'a tauri::AppHandle);
impl LoginRegistration for NativeLogin<'_> {
    fn is_enabled(&self) -> Result<bool, String> {
        self.0.autolaunch().is_enabled().map_err(|e| e.to_string())
    }
    fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        let manager = self.0.autolaunch();
        if enabled {
            manager.enable()
        } else {
            manager.disable()
        }
        .map_err(|e| e.to_string())
    }
}

pub(crate) fn load(
    dir: &Path,
    login: &impl LoginRegistration,
) -> Result<(Config, Option<String>), String> {
    let mut config = store::load(dir)?;
    // Read-only: respect login-item changes made outside OpenStreamDeck.
    let error = match login.is_enabled() {
        Ok(enabled) => {
            config.launch_at_login = enabled;
            None
        }
        Err(error) => Some(format!("Could not read launch at login: {error}")),
    };
    Ok((config, error))
}

fn set_verified(login: &impl LoginRegistration, enabled: bool) -> Result<(), String> {
    login.set_enabled(enabled)?;
    if login.is_enabled()? != enabled {
        return Err("The operating system did not apply the login preference".into());
    }
    Ok(())
}

/// Caller holds the config lock. Roll back OS changes if registration or saving
/// fails, and never publish an unsuccessful preference as live configuration.
pub(crate) fn save(
    dir: &Path,
    current: &mut Config,
    next: Config,
    login: &impl LoginRegistration,
) -> Result<(), String> {
    next.validate()?;
    let previous = login
        .is_enabled()
        .map_err(|e| format!("Could not read launch at login: {e}"))?;
    let changed = previous != next.launch_at_login;
    let result = if changed {
        set_verified(login, next.launch_at_login)
            .map_err(|e| format!("Could not update launch at login: {e}"))
    } else {
        Ok(())
    }
    .and_then(|()| store::save(dir, &next));
    if let Err(error) = result {
        if changed {
            if let Err(rollback) = set_verified(login, previous) {
                return Err(format!("{error}. Restoring launch at login also failed: {rollback}. Check your operating system's login items."));
            }
        }
        return Err(error);
    }
    *current = next;
    Ok(())
}

pub(crate) fn keep_running_after_exit(menu_bar_only: bool, code: Option<i32>) -> bool {
    menu_bar_only && code.is_none()
}

pub(crate) fn start_in_background(menu_bar_only: bool, args: &[String]) -> bool {
    args.iter()
        .any(|arg| arg == "--background" || arg == "--hardware-smoke")
        || (menu_bar_only && args.iter().any(|arg| arg == "--from-autostart"))
}

pub(crate) trait CloseHost {
    fn background(&self) -> Result<(), String>;
    fn quit(&self);
}

pub(crate) fn window_closed(menu_bar_only: bool, host: &impl CloseHost) -> Result<(), String> {
    if menu_bar_only {
        host.background()
    } else {
        host.quit();
        Ok(())
    }
}

pub(crate) struct NativeClose<'a>(pub &'a tauri::AppHandle);
impl CloseHost for NativeClose<'_> {
    fn background(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        self.0
            .set_activation_policy(tauri::ActivationPolicy::Accessory)
            .map_err(|e| format!("Could not enter menu bar mode: {e}"))?;
        Ok(())
    }
    fn quit(&self) {
        self.0.exit(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[derive(Default)]
    struct Login {
        enabled: Cell<bool>,
        writes: RefCell<Vec<bool>>,
        fail_read: Cell<bool>,
        fail_write: Cell<bool>,
        ignore_write: Cell<bool>,
        fail_verification: Cell<bool>,
    }
    impl LoginRegistration for Login {
        fn is_enabled(&self) -> Result<bool, String> {
            if self.fail_read.get()
                || (!self.writes.borrow().is_empty() && self.fail_verification.replace(false))
            {
                Err("registration unreadable".into())
            } else {
                Ok(self.enabled.get())
            }
        }
        fn set_enabled(&self, enabled: bool) -> Result<(), String> {
            self.writes.borrow_mut().push(enabled);
            if self.fail_write.get() {
                return Err("registration denied".into());
            }
            if !self.ignore_write.get() {
                self.enabled.set(enabled);
            }
            Ok(())
        }
    }

    #[test]
    fn startup_preserves_application_page_settings_through_save_and_reload() {
        // Public synthetic data matching the other current-version writer.
        // Exercise the same loader called by Tauri's setup hook.
        let bytes = include_bytes!("../../tests/fixtures/config-application-pages.json");
        let original: serde_json::Value = serde_json::from_slice(bytes).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, bytes).unwrap();
        let login = Login::default();

        let (mut current, warning) = load(dir.path(), &login)
            .expect("startup must accept saved version-2 application-page settings");
        assert!(warning.is_none());
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(serde_json::to_value(&current).unwrap(), original);
        assert!(login.writes.borrow().is_empty());

        // Model an ordinary editor save through the JSON IPC boundary.
        let mut ipc = serde_json::to_value(&current).unwrap();
        ipc["brightness"] = serde_json::json!(42);
        let next = Config::decode(&serde_json::to_vec(&ipc).unwrap()).unwrap();
        save(dir.path(), &mut current, next, &login).unwrap();
        let (reloaded, _) = load(dir.path(), &login).unwrap();
        assert_eq!(serde_json::to_value(&reloaded).unwrap(), ipc);
        assert_eq!(
            std::fs::read(dir.path().join("config.backup.json")).unwrap(),
            bytes
        );
        login.fail_read.set(true);
        let (_, warning) = load(dir.path(), &login).unwrap();
        let warning = warning.unwrap();
        assert!(warning.contains("Could not read launch at login"));
        assert!(!warning.contains("page switching is unavailable"));
    }

    #[test]
    fn startup_reads_os_registration_without_rewriting_preferences() {
        let dir = tempfile::tempdir().unwrap();
        for saved in [false, true] {
            let config = Config {
                launch_at_login: saved,
                sleep_minutes: 27,
                ..Default::default()
            };
            store::save(dir.path(), &config).unwrap();
            let login = Login {
                enabled: Cell::new(!saved),
                ..Default::default()
            };
            let (loaded, error) = load(dir.path(), &login).unwrap();
            assert_eq!(loaded.launch_at_login, !saved);
            assert_eq!(loaded.sleep_minutes, 27);
            assert!(error.is_none());
            assert!(login.writes.borrow().is_empty());
            assert_eq!(store::load(dir.path()).unwrap().launch_at_login, saved);
            login.fail_read.set(true);
            let (loaded, error) = load(dir.path(), &login).unwrap();
            assert_eq!(loaded.launch_at_login, saved);
            assert!(error.unwrap().contains("Could not read launch at login"));
        }
    }

    #[test]
    fn login_enables_disables_and_round_trips_both_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let login = Login::default();
        let mut current = Config::default();
        for enabled in [true, false, false, true] {
            let next = Config {
                launch_at_login: enabled,
                menu_bar_only: !enabled,
                ..current.clone()
            };
            save(dir.path(), &mut current, next, &login).unwrap();
            assert_eq!(login.enabled.get(), enabled);
            assert_eq!(current.launch_at_login, enabled);
            let loaded = store::load(dir.path()).unwrap();
            assert_eq!(loaded.launch_at_login, enabled);
            assert_eq!(loaded.menu_bar_only, !enabled);
        }
        assert_eq!(*login.writes.borrow(), [true, false, true]);
    }

    #[test]
    fn failures_leave_live_and_persisted_preferences_unchanged() {
        for enabled in [false, true] {
            for failure in ["read", "write", "ignored", "verification", "save"] {
                let dir = tempfile::tempdir().unwrap();
                let mut current = Config {
                    launch_at_login: enabled,
                    ..Default::default()
                };
                store::save(dir.path(), &current).unwrap();
                let login = Login {
                    enabled: Cell::new(enabled),
                    ..Default::default()
                };
                match failure {
                    "read" => login.fail_read.set(true),
                    "write" => login.fail_write.set(true),
                    "ignored" => login.ignore_write.set(true),
                    "verification" => login.fail_verification.set(true),
                    "save" => std::fs::create_dir(dir.path().join("config.tmp")).unwrap(),
                    _ => unreachable!(),
                }
                let next = Config {
                    launch_at_login: !enabled,
                    menu_bar_only: false,
                    ..current.clone()
                };
                let error = save(dir.path(), &mut current, next, &login).unwrap_err();
                assert!(!error.is_empty());
                if failure == "write" {
                    assert!(error.contains("Restoring launch at login also failed"));
                }
                assert_eq!(login.enabled.get(), enabled, "{failure}");
                assert_eq!(current.launch_at_login, enabled);
                assert!(current.menu_bar_only);
                assert_eq!(store::load(dir.path()).unwrap().launch_at_login, enabled);
            }
        }
    }

    #[test]
    fn invalid_settings_have_no_os_or_persistence_effects() {
        let dir = tempfile::tempdir().unwrap();
        let login = Login::default();
        let mut current = Config::default();
        let next = Config {
            sleep_minutes: 1441,
            launch_at_login: true,
            ..current.clone()
        };
        assert!(save(dir.path(), &mut current, next, &login).is_err());
        assert!(login.writes.borrow().is_empty());
        assert!(!dir.path().join("config.json").exists());
    }

    #[test]
    fn sleep_defaults_off_and_existing_preferences_are_preserved() {
        let mut value = serde_json::to_value(Config::default()).unwrap();
        assert_eq!(value["sleep_minutes"], 0);
        value.as_object_mut().unwrap().remove("launch_at_login");
        value.as_object_mut().unwrap().remove("menu_bar_only");
        value["sleep_minutes"] = 15.into();
        let config: Config = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(config.sleep_minutes, 15);
        assert!(!config.launch_at_login);
        assert!(config.menu_bar_only);
        for key in ["launch_at_login", "menu_bar_only"] {
            for invalid in [serde_json::Value::Null, "true".into(), 1.into()] {
                let mut bad = value.clone();
                bad[key] = invalid;
                assert!(serde_json::from_value::<Config>(bad).is_err());
            }
        }
    }

    #[derive(Default)]
    struct Close {
        calls: RefCell<Vec<&'static str>>,
        fail: Cell<bool>,
    }
    impl CloseHost for Close {
        fn background(&self) -> Result<(), String> {
            self.calls.borrow_mut().push("background");
            if self.fail.get() {
                Err("OS refused background mode".into())
            } else {
                Ok(())
            }
        }
        fn quit(&self) {
            self.calls.borrow_mut().push("quit");
        }
    }
    #[test]
    fn closing_uses_current_preference_and_explicit_quit_always_exits() {
        let host = Close::default();
        for mode in [true, false, true] {
            window_closed(mode, &host).unwrap();
            assert_eq!(keep_running_after_exit(mode, None), mode);
            assert!(!keep_running_after_exit(mode, Some(0)));
            assert!(!keep_running_after_exit(mode, Some(1)));
        }
        assert_eq!(*host.calls.borrow(), ["background", "quit", "background"]);
        host.fail.set(true);
        assert_eq!(
            window_closed(true, &host).unwrap_err(),
            "OS refused background mode"
        );
        window_closed(false, &host).unwrap();
    }
    #[test]
    fn login_startup_respects_tray_mode_and_manual_launch_opens_window() {
        for mode in [false, true] {
            assert_eq!(
                start_in_background(mode, &["--from-autostart".into()]),
                mode
            );
            assert!(!start_in_background(mode, &[]));
            assert!(start_in_background(mode, &["--background".into()]));
            assert!(start_in_background(mode, &["--hardware-smoke".into()]));
        }
    }
}
