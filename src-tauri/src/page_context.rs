//! Current application/page selection. Automatic changes never mutate saved config.
use crate::{foreground::ForegroundApplication, model::*, AppState};
use serde::Serialize;
use std::sync::{atomic::Ordering, Arc};
use tauri::{Emitter, Manager};

#[derive(Clone, Debug, Serialize)]
pub struct PageContext {
    pub revision: u64,
    pub desired_page: String,
    pub applied_page: Option<String>,
    pub reason: String,
    pub application: Option<ForegroundApplication>,
    pub error: Option<String>,
    pub automatic: bool,
    pub override_active: bool,
    pub overlay: bool,
    pub overlay_applied: bool,
    #[serde(skip)]
    pub generation: u64,
    #[serde(skip)]
    pub observer_epoch: u64,
    #[serde(skip)]
    pub overlay_epoch: u64,
    #[serde(skip)]
    applied_generation: Option<u64>,
    #[serde(skip)]
    missing_since: Option<u64>,
}

impl PageContext {
    pub fn new(config: &Config) -> Self {
        let mut context = Self {
            revision: 0,
            desired_page: config.active_page.clone(),
            applied_page: None,
            reason: "manual".into(),
            application: None,
            error: None,
            automatic: config.page_switching.enabled,
            override_active: false,
            overlay: false,
            overlay_applied: false,
            generation: 0,
            observer_epoch: 0,
            overlay_epoch: 0,
            applied_generation: None,
            missing_since: None,
        };
        context.resolve(config);
        context
    }

    pub fn effective_config(&self, config: &Config) -> Config {
        let mut effective = config.clone();
        effective.active_page = self.desired_page.clone();
        effective
    }

    fn resolve(&mut self, config: &Config) {
        if self.override_active && config.pages.iter().any(|p| p.id == self.desired_page) {
            return;
        }
        self.override_active = false;
        let (page, reason) = if self.automatic {
            // A distinct activation enters the first child in saved array order.
            // Manual child selection is preserved above until the app changes.
            let matched = self.application.as_ref().and_then(|app| {
                config.pages.iter().find(|page| {
                    page.application.as_ref().is_some_and(|association| {
                        association
                            .identities
                            .iter()
                            .any(|id| id.key() == app.identity.key())
                    })
                })
            });
            match matched {
                Some(page) => (page.id.clone(), "application"),
                None => (
                    config
                        .page_switching
                        .default_page
                        .clone()
                        .unwrap_or_else(|| config.active_page.clone()),
                    "default",
                ),
            }
        } else {
            (config.active_page.clone(), "manual")
        };
        if self.desired_page != page {
            self.desired_page = page;
            self.generation += 1;
        }
        self.reason = reason.into();
    }

    /// Called only after a successful save, under the config -> context lock order.
    pub fn configure(&mut self, config: &Config) {
        if self.automatic != config.page_switching.enabled {
            self.observer_epoch += 1;
            self.automatic = config.page_switching.enabled;
            self.application = None;
            self.error = None;
            self.missing_since = None;
            self.override_active = false;
        }
        self.resolve(config);
        // Contents or display settings may have changed without changing page ID.
        self.generation += 1;
        self.overlay_epoch += 1;
        self.revision += 1;
    }

    pub fn observe(
        &mut self,
        config: &Config,
        observation: Result<Option<ForegroundApplication>, String>,
        now_ms: u64,
    ) -> bool {
        if !self.automatic {
            return false;
        }
        let (application, error) = match observation {
            Ok(Some(app)) => {
                self.missing_since = None;
                (Some(app), None)
            }
            Ok(None) => {
                self.missing_since.get_or_insert(now_ms);
                return self.tick(config, now_ms);
            }
            Err(error) => {
                self.missing_since = None;
                (None, Some(error))
            }
        };
        if self.application == application && self.error == error {
            return false;
        }
        let identity_changed = self.application.as_ref().map(|a| a.identity.key())
            != application.as_ref().map(|a| a.identity.key());
        if identity_changed || self.error.is_some() != error.is_some() {
            self.override_active = false;
        }
        self.application = application;
        self.error = error;
        self.resolve(config);
        self.revision += 1;
        true
    }

    pub fn tick(&mut self, config: &Config, now_ms: u64) -> bool {
        if self
            .missing_since
            .is_some_and(|since| now_ms.saturating_sub(since) >= 150)
        {
            self.missing_since = None;
            return self.observe(
                config,
                Err("No foreground application is available".into()),
                now_ms,
            );
        }
        false
    }

    pub fn manual(&mut self, config: &Config, page: &str) -> Result<(), String> {
        if !config.pages.iter().any(|p| p.id == page) {
            return Err("Page not found".into());
        }
        if self.desired_page != page {
            self.generation += 1;
        }
        self.desired_page = page.into();
        self.reason = "manual".into();
        self.override_active = self.automatic;
        self.set_overlay(false);
        self.revision += 1;
        Ok(())
    }

    pub fn resume(&mut self, config: &Config) {
        self.override_active = false;
        self.resolve(config);
        self.revision += 1;
    }

    pub fn set_overlay(&mut self, overlay: bool) {
        if self.overlay != overlay {
            self.overlay = overlay;
            self.overlay_epoch += 1;
            self.generation += 1;
            self.revision += 1;
        }
    }

    pub fn invalidate(&mut self) {
        self.applied_page = None;
        self.overlay_applied = false;
        self.applied_generation = None;
        self.generation += 1;
        self.revision += 1;
    }

    pub fn needs_frame(&self) -> bool {
        !self.overlay && self.applied_generation != Some(self.generation)
    }

    pub fn applied(&mut self, page: String, generation: u64) {
        self.applied_page = Some(page);
        self.overlay_applied = false;
        self.applied_generation = Some(generation);
        self.revision += 1;
    }

    pub fn applied_overlay(&mut self) {
        self.applied_page = None;
        self.applied_generation = None;
        self.overlay_applied = true;
        self.revision += 1;
    }
}

pub fn publish(app: &tauri::AppHandle) {
    let context = app.state::<AppState>().page_context.lock().unwrap().clone();
    let _ = app.emit("page-context", context);
}

/// Manual page actions and IPC share this transaction. Only deliberate manual
/// selection persists active_page. Navigation starts from the actual target page.
pub fn select(app: &tauri::AppHandle, value: &str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let saved = select_state(&state, value)?;
    let _ = app.emit("config", saved);
    publish(app);
    Ok(())
}

pub(crate) fn select_state(state: &AppState, value: &str) -> Result<Config, String> {
    // Page selection persists configuration too. Share installation's gate and
    // lock order so queued IPC cannot write during download/install/restart.
    let _active = state.active.lock().unwrap();
    if state.update_installing.load(Ordering::SeqCst) {
        return Err("An update is in progress. Pages cannot be changed until it finishes.".into());
    }
    let mut config = state.config.lock().unwrap();
    let mut context = state.page_context.lock().unwrap();
    select_saved(&state.dir, &mut config, &mut context, value)?;
    Ok(config.clone())
}

fn select_saved(
    dir: &std::path::Path,
    config: &mut Config,
    context: &mut PageContext,
    value: &str,
) -> Result<(), String> {
    let mut next = config.clone();
    next.active_page = context.desired_page.clone();
    crate::engine::switch_page(&mut next, value)?;
    crate::store::save(dir, &next)?;
    context.manual(&next, &next.active_page)?;
    *config = next;
    Ok(())
}

/// The observer belongs to the native process, never a webview subscription.
pub fn monitor(app: tauri::AppHandle) {
    let origin = std::time::Instant::now();
    let mut observer = None;
    let mut epoch = u64::MAX;
    let mut retry = std::time::Instant::now();
    loop {
        let state = app.state::<AppState>();
        if state.quitting.load(Ordering::Relaxed) {
            break;
        }
        let context = state.page_context.lock().unwrap().clone();
        if !context.automatic || epoch != context.observer_epoch {
            observer = None;
            epoch = context.observer_epoch;
            retry = std::time::Instant::now();
        }
        if context.automatic && observer.is_none() && std::time::Instant::now() >= retry {
            let handle = app.clone();
            let callback = Arc::new(move |observation| {
                let state = handle.state::<AppState>();
                let config = state.config.lock().unwrap();
                let mut context = state.page_context.lock().unwrap();
                if !context.automatic || context.observer_epoch != epoch {
                    return;
                }
                let changed =
                    context.observe(&config, observation, origin.elapsed().as_millis() as u64);
                drop(context);
                drop(config);
                if changed {
                    publish(&handle);
                }
            });
            match crate::foreground::start(callback) {
                Ok(handle) => observer = Some(handle),
                Err(error) => {
                    let config = state.config.lock().unwrap();
                    let mut context = state.page_context.lock().unwrap();
                    let changed =
                        context.observe(&config, Err(error), origin.elapsed().as_millis() as u64);
                    drop(context);
                    drop(config);
                    if changed {
                        publish(&app);
                    }
                }
            }
            retry = std::time::Instant::now() + std::time::Duration::from_secs(3);
        }
        let config = state.config.lock().unwrap();
        let changed = state
            .page_context
            .lock()
            .unwrap()
            .tick(&config, origin.elapsed().as_millis() as u64);
        drop(config);
        if changed {
            publish(&app);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app(value: &str) -> ForegroundApplication {
        ForegroundApplication {
            name: value.into(),
            identity: ApplicationIdentity {
                platform: "macos".into(),
                value: value.into(),
            },
        }
    }
    fn config() -> Config {
        let mut config = Config {
            page_switching: PageSwitching {
                enabled: true,
                default_page: Some("home".into()),
            },
            ..Default::default()
        };
        for name in ["chrome", "code"] {
            config.pages.push(Page {
                id: name.into(),
                name: name.into(),
                bindings: Default::default(),
                application: Some(ApplicationAssociation {
                    name: name.into(),
                    identities: vec![app(name).identity],
                    image: None,
                }),
            });
        }
        config
    }
    #[test]
    fn application_default_and_manual_policy_never_changes_saved_configuration() {
        let config = config();
        let original = serde_json::to_vec(&config).unwrap();
        let mut state = PageContext::new(&config);
        for (application, page) in [
            ("chrome", "chrome"),
            ("code", "code"),
            ("other", "home"),
            ("chrome", "chrome"),
        ] {
            assert!(state.observe(&config, Ok(Some(app(application))), 0));
            assert_eq!(state.desired_page, page);
        }
        state.manual(&config, "code").unwrap();
        assert!(!state.observe(&config, Ok(Some(app("chrome"))), 0));
        assert_eq!(state.desired_page, "code");
        state.observe(&config, Ok(Some(app("other"))), 0);
        assert_eq!(state.desired_page, "home");
        state.manual(&config, "code").unwrap();
        state.observe(&config, Ok(Some(app("another"))), 0);
        assert_eq!(state.desired_page, "home");
        assert!(!state.override_active);
        assert_eq!(serde_json::to_vec(&config).unwrap(), original);
    }
    #[test]
    fn missing_observation_grace_and_unavailable_recovery_use_injected_time() {
        let config = config();
        let mut state = PageContext::new(&config);
        state.observe(&config, Ok(Some(app("chrome"))), 0);
        state.observe(&config, Ok(None), 10);
        assert!(!state.tick(&config, 159));
        assert_eq!(state.desired_page, "chrome");
        state.observe(&config, Ok(Some(app("code"))), 160);
        assert!(!state.tick(&config, 1000));
        state.observe(&config, Ok(None), 1100);
        assert!(state.tick(&config, 1250));
        assert_eq!(state.desired_page, "home");
        state.manual(&config, "code").unwrap();
        // Repeated failures do not fight the user's manual selection.
        assert!(!state.observe(
            &config,
            Err("No foreground application is available".into()),
            1400
        ));
        assert!(state.observe(&config, Err("Adapter retry is unavailable".into()), 1450));
        assert_eq!(state.desired_page, "code");
        assert!(state.override_active);
        state.observe(&config, Ok(Some(app("chrome"))), 1500);
        assert_eq!(state.desired_page, "chrome");
        assert!(state.error.is_none());
    }
    #[test]
    fn revisions_overlay_stale_frames_and_deleted_override() {
        let mut config = config();
        let mut state = PageContext::new(&config);
        state.applied("home".into(), state.generation);
        assert!(!state.needs_frame());
        let old = state.generation;
        state.observe(&config, Ok(Some(app("chrome"))), 0);
        state.applied("home".into(), old);
        assert!(state.needs_frame());
        state.set_overlay(true);
        assert!(!state.needs_frame());
        state.observe(&config, Ok(Some(app("code"))), 0);
        assert_eq!(state.desired_page, "code");
        state.set_overlay(false);
        assert!(state.needs_frame());
        state.manual(&config, "chrome").unwrap();
        config.pages.retain(|p| p.id != "chrome");
        state.configure(&config);
        assert_eq!(state.desired_page, "code");
        assert!(!state.override_active);
        config.page_switching.enabled = false;
        state.configure(&config);
        assert_eq!(state.desired_page, config.active_page);
        assert!(!state.observe(&config, Ok(Some(app("code"))), 0));
        assert!(state.manual(&config, "missing").is_err());
    }

    #[test]
    fn overlay_requested_and_transmitted_status_are_independent() {
        let config = config();
        let mut context = PageContext::new(&config);
        context.applied("home".into(), context.generation);
        context.set_overlay(true);
        assert!(context.overlay);
        assert!(!context.overlay_applied);
        assert_eq!(context.applied_page.as_deref(), Some("home"));
        context.applied_overlay();
        assert!(context.overlay_applied);
        assert!(context.applied_page.is_none());
        context.manual(&config, "chrome").unwrap();
        assert!(!context.overlay);
        assert!(context.overlay_applied);
        assert!(context.needs_frame());
        context.applied("chrome".into(), context.generation);
        assert!(!context.overlay_applied);
        assert_eq!(context.applied_page.as_deref(), Some("chrome"));
        context.set_overlay(true);
        context.applied_overlay();
        context.invalidate();
        assert!(!context.overlay_applied);
        assert!(context.applied_page.is_none());
    }

    #[test]
    fn overlay_requests_and_manual_selection_share_one_authoritative_mode() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let state = AppState::new(dir.path().into(), config(), tx);
        // Both possible serialized orderings converge on the last operation.
        // The producer and worker read this same mode; no independent flag can
        // strand the worker in overlay mode while stopping overlay production.
        state.page_context.lock().unwrap().set_overlay(true);
        {
            let mut config = state.config.lock().unwrap();
            let mut context = state.page_context.lock().unwrap();
            select_saved(dir.path(), &mut config, &mut context, "chrome").unwrap();
        }
        assert!(!state.page_context.lock().unwrap().overlay);
        assert!(state.page_context.lock().unwrap().needs_frame());
        state.page_context.lock().unwrap().set_overlay(true);
        assert!(state.page_context.lock().unwrap().overlay);
        assert!(!state.page_context.lock().unwrap().needs_frame());
        assert_eq!(state.page_context.lock().unwrap().desired_page, "chrome");
        state.page_context.lock().unwrap().set_overlay(false);
        assert!(state.page_context.lock().unwrap().needs_frame());
    }

    #[test]
    fn manual_navigation_persists_atomically_and_starts_from_automatic_page() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config();
        let mut second = config
            .pages
            .iter()
            .find(|page| page.id == "chrome")
            .unwrap()
            .clone();
        second.id = "chrome-tools".into();
        second.name = "Chrome tools".into();
        config.pages.push(second);
        crate::store::save(dir.path(), &config).unwrap();
        let mut state = PageContext::new(&config);
        state.observe(&config, Ok(Some(app("chrome"))), 0);
        select_saved(dir.path(), &mut config, &mut state, "next").unwrap();
        assert_eq!(config.active_page, "chrome-tools");
        assert_eq!(state.desired_page, "chrome-tools");
        assert!(state.override_active);
        select_saved(dir.path(), &mut config, &mut state, "next").unwrap();
        assert_eq!(state.desired_page, "chrome");
        select_saved(dir.path(), &mut config, &mut state, "previous").unwrap();
        assert_eq!(state.desired_page, "chrome-tools");
        std::fs::create_dir(dir.path().join("config.tmp")).unwrap();
        let revision = state.revision;
        assert!(select_saved(dir.path(), &mut config, &mut state, "previous").is_err());
        assert_eq!(state.revision, revision);
        assert_eq!(state.desired_page, "chrome-tools");
        assert_eq!(
            crate::store::load(dir.path()).unwrap().active_page,
            "chrome-tools"
        );
        assert!(select_saved(dir.path(), &mut config, &mut state, "missing").is_err());
    }

    #[test]
    fn app_activation_enters_first_saved_child_and_reorder_preserves_manual_override() {
        let mut config = config();
        let mut second = config
            .pages
            .iter()
            .find(|page| page.id == "chrome")
            .unwrap()
            .clone();
        second.id = "chrome-tools".into();
        second.name = "Chrome tools".into();
        config.pages.push(second);
        let mut context = PageContext::new(&config);
        context.observe(&config, Ok(Some(app("chrome"))), 0);
        assert_eq!(context.desired_page, "chrome");
        context.manual(&config, "chrome-tools").unwrap();
        assert!(!context.observe(&config, Ok(Some(app("chrome"))), 1));
        assert_eq!(context.desired_page, "chrome-tools");
        context.observe(&config, Ok(Some(app("code"))), 2);
        assert_eq!(context.desired_page, "code");
        context.observe(&config, Ok(Some(app("chrome"))), 3);
        assert_eq!(context.desired_page, "chrome");
        assert!(!context.override_active);
        context.manual(&config, "chrome").unwrap();
        config.pages.swap(1, 3);
        context.configure(&config);
        assert_eq!(context.desired_page, "chrome");
        assert!(context.override_active);
        context.resume(&config);
        assert_eq!(context.desired_page, "chrome-tools");
        // Default is a designated role, never whichever ordinary page sorts first.
        let mut ordinary = config.pages[0].clone();
        ordinary.id = "utilities".into();
        config.pages.insert(0, ordinary);
        context.observe(&config, Ok(Some(app("unassigned"))), 4);
        assert_eq!(context.desired_page, "home");
        context.observe(&config, Ok(Some(app("chrome"))), 5);
        assert_eq!(context.desired_page, "chrome-tools");
        config.pages.retain(|page| page.id != "chrome-tools");
        context.configure(&config);
        assert_eq!(context.desired_page, "chrome");
        config.pages.retain(|page| page.id != "chrome");
        context.configure(&config);
        assert_eq!(context.desired_page, "home");
    }

    #[test]
    fn saved_reordering_controls_scoped_navigation_after_reload() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = config();
        let mut second = config.pages[1].clone();
        second.id = "chrome-tools".into();
        config.pages.insert(1, second);
        crate::store::save(dir.path(), &config).unwrap();
        let mut config = crate::store::load(dir.path()).unwrap();
        let mut context = PageContext::new(&config);
        context.observe(&config, Ok(Some(app("chrome"))), 0);
        assert_eq!(context.desired_page, "chrome-tools");
        select_saved(dir.path(), &mut config, &mut context, "next").unwrap();
        assert_eq!(context.desired_page, "chrome");
        assert_eq!(
            crate::store::load(dir.path()).unwrap().active_page,
            "chrome"
        );
        select_saved(dir.path(), &mut config, &mut context, "next").unwrap();
        assert_eq!(context.desired_page, "chrome-tools");
        // Explicit manual selection can enter the ordinary context, which excludes apps.
        select_saved(dir.path(), &mut config, &mut context, "home").unwrap();
        select_saved(dir.path(), &mut config, &mut context, "previous").unwrap();
        assert_eq!(context.desired_page, "home");
        assert!(context.override_active);
    }

    #[test]
    fn automatic_events_do_not_touch_files_and_resume_uses_current_app() {
        let dir = tempfile::tempdir().unwrap();
        let config = config();
        crate::store::save(dir.path(), &config).unwrap();
        let bytes = std::fs::read(dir.path().join("config.json")).unwrap();
        let mut context = PageContext::new(&config);
        for value in ["chrome", "code", "missing", "chrome"] {
            context.observe(&config, Ok(Some(app(value))), 0);
        }
        context.manual(&config, "code").unwrap();
        context.resume(&config);
        assert_eq!(context.desired_page, "chrome");
        assert!(!context.override_active);
        assert_eq!(
            std::fs::read(dir.path().join("config.json")).unwrap(),
            bytes
        );
        assert!(!dir.path().join("config.backup.json").exists());
    }
}
