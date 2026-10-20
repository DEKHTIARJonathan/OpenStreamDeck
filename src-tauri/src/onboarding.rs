//! Installation-local setup state, separate from portable configuration.
use crate::{model::Config, settings};
use serde::{Deserialize, Serialize};
use std::{io::Write, path::Path};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Setup {
    pub completed: bool,
}

fn persist(dir: &Path, setup: &Setup) -> Result<(), String> {
    let write = || -> Result<(), Box<dyn std::error::Error>> {
        let mut file = std::fs::File::create(dir.join("onboarding.tmp"))?;
        file.write_all(&serde_json::to_vec(setup)?)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(dir.join("onboarding.tmp"), dir.join("onboarding.json"))?;
        Ok(())
    };
    write().map_err(|e| format!("Could not remember setup: {e}"))
}

pub(crate) struct State {
    setup: Result<Setup, String>,
    existing_installation: Option<bool>,
}

impl State {
    pub(crate) fn new(dir: &Path) -> Self {
        let mut existing_installation = None;
        let setup = load(dir, &mut existing_installation);
        Self {
            setup,
            existing_installation,
        }
    }

    pub(crate) fn pending(&self) -> bool {
        self.setup.as_ref().map_or(true, |setup| !setup.completed)
    }

    pub(crate) fn setup(&mut self, dir: &Path) -> Result<&mut Setup, String> {
        if self.setup.is_err() {
            self.setup = load(dir, &mut self.existing_installation);
        }
        self.setup.as_mut().map_err(|error| error.clone())
    }
}

fn load(dir: &Path, existing_installation: &mut Option<bool>) -> Result<Setup, String> {
    match std::fs::read(dir.join("onboarding.json")) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|e| format!("Could not read setup state: {e}"))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Capture this before any onboarding save can create config.json.
            // An update must never onboard an existing installation.
            let completed = match *existing_installation {
                Some(existing) => existing,
                None => {
                    let existing = dir
                        .join("config.json")
                        .try_exists()
                        .map_err(|e| e.to_string())?
                        || dir
                            .join("config.backup.json")
                            .try_exists()
                            .map_err(|e| e.to_string())?;
                    // Retain the startup classification even when writing the
                    // marker fails and a background action later saves config.
                    *existing_installation = Some(existing);
                    existing
                }
            };
            let setup = Setup { completed };
            persist(dir, &setup)?;
            Ok(setup)
        }
        Err(e) => Err(format!("Could not read setup state: {e}")),
    }
}

pub(crate) fn finish(
    dir: &Path,
    setup: &mut Setup,
    config: &mut Config,
    login: &impl settings::LoginRegistration,
    enable_login: bool,
) -> Result<(), String> {
    if setup.completed {
        return Ok(());
    }
    if enable_login {
        let next = Config {
            launch_at_login: true,
            ..config.clone()
        };
        settings::save(dir, config, next, login)?;
    }
    let next = Setup { completed: true };
    persist(dir, &next)?;
    *setup = next;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InputAccess {
    Granted,
    Required,
    NotRequired,
    Limited,
}

pub(crate) fn input_policy(platform: &str, wayland: bool, trusted: bool) -> InputAccess {
    match platform {
        "macos" if trusted => InputAccess::Granted,
        "macos" => InputAccess::Required,
        "linux" if wayland => InputAccess::Limited,
        _ => InputAccess::NotRequired,
    }
}

pub(crate) trait PermissionHost {
    fn trusted(&self) -> bool;
    fn open_settings(&self) -> Result<(), String>;
}

pub(crate) struct NativePermission;
impl PermissionHost for NativePermission {
    fn trusted(&self) -> bool {
        #[cfg(target_os = "macos")]
        // SAFETY: read-only OS trust query, with no arguments or owned pointers.
        unsafe {
            macos::AXIsProcessTrusted() != 0
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }
    fn open_settings(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        {
            macos::prompt()?;
            open::that(
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
            )
            .map_err(|e| format!("Could not open Accessibility settings: {e}"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            Err("This operating system does not use macOS Accessibility permission.".into())
        }
    }
}

pub(crate) fn request_access(platform: &str, host: &impl PermissionHost) -> Result<(), String> {
    if platform != "macos" {
        return Err("This operating system does not use macOS Accessibility permission.".into());
    }
    host.open_settings()
}

#[derive(Serialize)]
pub(crate) struct Status {
    completed: bool,
    platform: &'static str,
    input_access: InputAccess,
    launch_at_login: Option<bool>,
    login_error: Option<String>,
}

pub(crate) fn status(setup: &Setup, login: &impl settings::LoginRegistration) -> Status {
    let platform = std::env::consts::OS;
    let registration = login.is_enabled();
    Status {
        completed: setup.completed,
        platform,
        input_access: input_policy(
            platform,
            std::env::var_os("WAYLAND_DISPLAY").is_some(),
            NativePermission.trusted(),
        ),
        launch_at_login: registration.as_ref().ok().copied(),
        login_error: registration
            .err()
            .map(|e| format!("Could not read launch at login: {e}")),
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::{ffi::c_void, ptr};
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        pub fn AXIsProcessTrusted() -> u8;
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
        static kAXTrustedCheckOptionPrompt: *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: *const c_void;
        fn CFDictionaryCreate(
            allocator: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            count: isize,
            key_callbacks: *const c_void,
            value_callbacks: *const c_void,
        ) -> *const c_void;
        fn CFRelease(value: *const c_void);
    }
    pub fn prompt() -> Result<(), String> {
        // SAFETY: static CoreFoundation key/value outlive the dictionary. Null
        // callbacks avoid ownership changes; the created dictionary is released.
        unsafe {
            let options = CFDictionaryCreate(
                ptr::null(),
                &kAXTrustedCheckOptionPrompt,
                &kCFBooleanTrue,
                1,
                ptr::null(),
                ptr::null(),
            );
            if options.is_null() {
                return Err("Could not request Accessibility access.".into());
            }
            // The result is deliberately ignored: prompting does not grant access.
            AXIsProcessTrustedWithOptions(options);
            CFRelease(options);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    fn load(dir: &Path) -> Result<Setup, String> {
        super::load(dir, &mut None)
    }

    #[test]
    fn initial_marker_failure_does_not_reclassify_after_background_save() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("onboarding.tmp")).unwrap();
        let mut state = State::new(dir.path());
        assert!(state.pending());
        assert!(state.setup(dir.path()).is_err());
        crate::store::save(dir.path(), &Config::default()).unwrap();
        std::fs::remove_dir(dir.path().join("onboarding.tmp")).unwrap();
        assert!(!state.setup(dir.path()).unwrap().completed);
        assert!(!load(dir.path()).unwrap().completed);
    }

    #[derive(Default)]
    struct Login {
        enabled: Cell<bool>,
        fail: Cell<bool>,
        writes: Cell<usize>,
    }
    impl settings::LoginRegistration for Login {
        fn is_enabled(&self) -> Result<bool, String> {
            Ok(self.enabled.get())
        }
        fn set_enabled(&self, enabled: bool) -> Result<(), String> {
            self.writes.set(self.writes.get() + 1);
            if self.fail.get() {
                return Err("registration denied".into());
            }
            self.enabled.set(enabled);
            Ok(())
        }
    }
    #[test]
    fn fresh_install_resumes_until_finished_and_skip_survives_config_changes() {
        let dir = tempfile::tempdir().unwrap();
        let mut setup = load(dir.path()).unwrap();
        assert!(!setup.completed);
        let mut config = Config::default();
        crate::store::save(dir.path(), &config).unwrap();
        assert!(!load(dir.path()).unwrap().completed);
        let login = Login::default();
        finish(dir.path(), &mut setup, &mut config, &login, false).unwrap();
        assert!(load(dir.path()).unwrap().completed);
        assert_eq!(login.writes.get(), 0);
        std::fs::remove_file(dir.path().join("config.json")).unwrap();
        assert!(load(dir.path()).unwrap().completed);
    }
    #[test]
    fn existing_installations_are_exempt_without_touching_configuration() {
        for filename in ["config.json", "config.backup.json"] {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join(filename), b"existing data").unwrap();
            assert!(load(dir.path()).unwrap().completed);
            assert_eq!(
                std::fs::read(dir.path().join(filename)).unwrap(),
                b"existing data"
            );
        }
    }
    #[test]
    fn enable_uses_verified_save_and_preserves_other_preferences() {
        let dir = tempfile::tempdir().unwrap();
        let mut setup = load(dir.path()).unwrap();
        let mut config = Config {
            brightness: 27,
            menu_bar_only: false,
            ..Default::default()
        };
        let login = Login::default();
        finish(dir.path(), &mut setup, &mut config, &login, true).unwrap();
        let saved = crate::store::load(dir.path()).unwrap();
        assert!(saved.launch_at_login);
        assert!(!saved.menu_bar_only);
        assert_eq!(saved.brightness, 27);
        assert!(load(dir.path()).unwrap().completed);
        finish(dir.path(), &mut setup, &mut config, &login, true).unwrap();
        assert_eq!(login.writes.get(), 1);
    }
    #[test]
    fn failures_remain_retryable_and_never_claim_completion() {
        for failure in ["login", "config", "marker"] {
            let dir = tempfile::tempdir().unwrap();
            let mut setup = load(dir.path()).unwrap();
            let mut config = Config::default();
            let login = Login::default();
            match failure {
                "login" => login.fail.set(true),
                "config" => std::fs::create_dir(dir.path().join("config.tmp")).unwrap(),
                _ => std::fs::create_dir(dir.path().join("onboarding.tmp")).unwrap(),
            }
            assert!(finish(
                dir.path(),
                &mut setup,
                &mut config,
                &login,
                failure != "marker"
            )
            .is_err());
            assert!(!setup.completed);
            assert!(!load(dir.path()).unwrap().completed);
            if failure != "marker" {
                assert!(!config.launch_at_login);
            }
        }
    }
    #[test]
    fn marker_failure_after_login_save_can_retry_without_losing_registration() {
        let dir = tempfile::tempdir().unwrap();
        let mut setup = load(dir.path()).unwrap();
        let mut config = Config::default();
        let login = Login::default();
        std::fs::create_dir(dir.path().join("onboarding.tmp")).unwrap();
        assert!(finish(dir.path(), &mut setup, &mut config, &login, true).is_err());
        assert!(!load(dir.path()).unwrap().completed);
        assert!(config.launch_at_login);
        assert!(crate::store::load(dir.path()).unwrap().launch_at_login);
        std::fs::remove_dir(dir.path().join("onboarding.tmp")).unwrap();
        // Finish without requesting a second login change, including after reload.
        finish(dir.path(), &mut setup, &mut config, &login, false).unwrap();
        assert!(load(dir.path()).unwrap().completed);
        assert_eq!(login.writes.get(), 1);
    }

    #[test]
    fn malformed_state_and_initial_write_errors_are_visible() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("onboarding.json"), b"bad").unwrap();
        assert!(load(dir.path())
            .unwrap_err()
            .contains("Could not read setup state"));
        std::fs::remove_file(dir.path().join("onboarding.json")).unwrap();
        std::fs::create_dir(dir.path().join("onboarding.tmp")).unwrap();
        assert!(load(dir.path())
            .unwrap_err()
            .contains("Could not remember setup"));
    }
    #[test]
    fn installation_gate_blocks_setup_and_login_effects_then_recovers() {
        let dir = tempfile::tempdir().unwrap();
        let mut setup = load(dir.path()).unwrap();
        let (tx, _rx) = std::sync::mpsc::sync_channel(8);
        let state = crate::AppState::new(dir.path().to_owned(), Config::default(), tx);
        let login = Login::default();
        state
            .update_installing
            .store(true, std::sync::atomic::Ordering::SeqCst);
        for enable in [false, true] {
            assert!(state
                .finish_setup(&mut setup, &login, enable)
                .unwrap_err()
                .contains("update is in progress"));
        }
        assert_eq!(login.writes.get(), 0);
        assert!(!load(dir.path()).unwrap().completed);
        state
            .update_installing
            .store(false, std::sync::atomic::Ordering::SeqCst);
        state.finish_setup(&mut setup, &login, true).unwrap();
        assert!(load(dir.path()).unwrap().completed);
        assert!(state.config.lock().unwrap().launch_at_login);
    }

    struct Permission {
        calls: Cell<usize>,
        fail: bool,
    }
    impl PermissionHost for Permission {
        fn trusted(&self) -> bool {
            false
        }
        fn open_settings(&self) -> Result<(), String> {
            self.calls.set(self.calls.get() + 1);
            if self.fail {
                Err("settings unavailable".into())
            } else {
                Ok(())
            }
        }
    }
    #[test]
    fn permission_routing_and_policy_never_infer_grant_from_opening_settings() {
        for fail in [false, true] {
            let host = Permission {
                calls: Cell::new(0),
                fail,
            };
            for platform in ["windows", "linux"] {
                assert!(request_access(platform, &host).is_err());
            }
            assert_eq!(host.calls.get(), 0);
            assert_eq!(request_access("macos", &host).is_err(), fail);
            assert_eq!(host.calls.get(), 1);
            assert_eq!(
                input_policy("macos", false, host.trusted()),
                InputAccess::Required
            );
        }
        assert_eq!(input_policy("macos", false, true), InputAccess::Granted);
        assert_eq!(
            input_policy("windows", false, false),
            InputAccess::NotRequired
        );
        assert_eq!(
            input_policy("linux", false, false),
            InputAccess::NotRequired
        );
        assert_eq!(input_policy("linux", true, false), InputAccess::Limited);
    }
}
