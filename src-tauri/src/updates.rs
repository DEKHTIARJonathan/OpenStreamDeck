//! Native update orchestration. Downloads stay in memory; only verified bytes reach installation.
use serde::Serialize;
use std::{
    future::Future,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tauri::{Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

mod history;

const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
const SCHEDULE_POLL: Duration = Duration::from_secs(60);

#[cfg(test)]
mod test_feed;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Idle,
    Checking,
    Current,
    Available,
    Downloading,
    Installing,
    Installed,
    Error,
}
#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub revision: u64,
    pub phase: Phase,
    pub current_version: String,
    pub version: Option<String>,
    pub notes: Option<String>,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub error: Option<String>,
    pub install_disabled: Option<String>,
    pub last_checked_at: Option<u64>,
    pub history_error: Option<String>,
    pub prompt_version: Option<String>,
}
struct Inner {
    status: Status,
    update: Option<Update>,
    history: history::History,
    next_check: tokio::time::Instant,
    checked_this_run: bool,
}
pub struct Service {
    inner: Mutex<Inner>,
    emit: Box<dyn Fn(&Status) + Send + Sync>,
    history_path: Option<PathBuf>,
    foreground: Box<dyn Fn() -> Result<(), String> + Send + Sync>,
}
impl Service {
    pub fn new(
        version: String,
        install_disabled: Option<String>,
        emit: impl Fn(&Status) + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner: Mutex::new(Inner {
                status: Status {
                    revision: 0,
                    phase: Phase::Idle,
                    current_version: version,
                    version: None,
                    notes: None,
                    downloaded: 0,
                    total: None,
                    error: None,
                    install_disabled,
                    last_checked_at: None,
                    history_error: None,
                    prompt_version: None,
                },
                update: None,
                history: history::History::default(),
                next_check: tokio::time::Instant::now(),
                checked_this_run: false,
            }),
            emit: Box::new(emit),
            history_path: None,
            foreground: Box::new(|| Ok(())),
        }
    }
    fn with_history(mut self, path: PathBuf) -> Self {
        let inner = self.inner.get_mut().unwrap();
        match history::load(&path) {
            Ok(history) => {
                inner.status.last_checked_at = history.last_checked_at;
                inner.history = history;
            }
            Err(error) => inner.status.history_error = Some(error),
        }
        self.history_path = Some(path);
        self
    }
    fn save_history(&self, history: &history::History) -> Result<(), String> {
        match &self.history_path {
            Some(path) => history::save(path, history),
            None => Ok(()),
        }
    }
    fn checked(&self, inner: &mut Inner) -> Status {
        inner.history.last_checked_at = Some(history::now_ms());
        inner.status.last_checked_at = inner.history.last_checked_at;
        inner.status.history_error = self.save_history(&inner.history).err();
        inner.next_check = tokio::time::Instant::now() + CHECK_INTERVAL;
        inner.checked_this_run = true;
        self.publish(inner)
    }
    fn check_failed(&self, error: String) -> Status {
        let mut inner = self.inner.lock().unwrap();
        inner.status.phase = Phase::Error;
        inner.status.error = Some(error);
        self.checked(&mut inner)
    }
    fn automatic_delay(&self) -> Duration {
        let inner = self.inner.lock().unwrap();
        if inner.status.prompt_version.is_some()
            || matches!(
                inner.status.phase,
                Phase::Checking | Phase::Downloading | Phase::Installing | Phase::Installed
            )
        {
            return SCHEDULE_POLL;
        }
        // Always check on process startup, even if a previous process checked recently.
        if !inner.checked_this_run {
            return Duration::ZERO;
        }
        let monotonic = inner
            .next_check
            .saturating_duration_since(tokio::time::Instant::now());
        // Wall time catches up after OS sleep; monotonic time tolerates clock rollback.
        let elapsed = Duration::from_millis(
            history::now_ms().saturating_sub(inner.history.last_checked_at.unwrap_or(0)),
        );
        monotonic
            .min(CHECK_INTERVAL.saturating_sub(elapsed))
            .min(SCHEDULE_POLL)
    }
    pub fn skip(&self, version: &str) -> Result<Status, String> {
        let mut inner = self.inner.lock().unwrap();
        if inner.status.prompt_version.as_deref() != Some(version) {
            return Err("This update notice is no longer current.".into());
        }
        let mut history = inner.history.clone();
        history.skipped_version = Some(version.to_owned());
        if let Err(error) = self.save_history(&history) {
            inner.status.history_error = Some(error.clone());
            self.publish(&mut inner);
            return Err(error);
        }
        inner.history = history;
        inner.status.history_error = None;
        inner.status.prompt_version = None;
        Ok(self.publish(&mut inner))
    }
    pub fn status(&self) -> Status {
        self.inner.lock().unwrap().status.clone()
    }
    // Publish while holding the lock: revision order is also event emission order.
    fn publish(&self, inner: &mut Inner) -> Status {
        inner.status.revision += 1;
        (self.emit)(&inner.status);
        inner.status.clone()
    }
    fn failed(&self, error: String) -> Status {
        let mut inner = self.inner.lock().unwrap();
        inner.status.phase = Phase::Error;
        inner.status.error = Some(error);
        self.publish(&mut inner)
    }
    async fn deadline<T>(
        duration: Duration,
        operation: impl Future<Output = Result<T, String>>,
    ) -> Result<T, String> {
        tokio::time::timeout(duration, operation)
            .await
            .unwrap_or_else(|_| {
                Err("Update request timed out. Check your connection and retry.".into())
            })
    }
    pub async fn check(
        &self,
        operation: impl Future<Output = Result<Option<Update>, String>>,
    ) -> Result<Status, String> {
        self.check_inner(operation, false).await
    }
    async fn check_inner(
        &self,
        operation: impl Future<Output = Result<Option<Update>, String>>,
        automatic: bool,
    ) -> Result<Status, String> {
        {
            let mut inner = self.inner.lock().unwrap();
            if inner.status.prompt_version.is_some() {
                return Err("Choose Update Now or Skip this Update before checking again.".into());
            }
            if matches!(
                inner.status.phase,
                Phase::Checking | Phase::Downloading | Phase::Installing | Phase::Installed
            ) {
                return Err("An update operation is already active.".into());
            }
            inner.status.phase = Phase::Checking;
            inner.status.error = None;
            inner.status.version = None;
            inner.status.notes = None;
            inner.status.prompt_version = None;
            inner.status.downloaded = 0;
            inner.status.total = None;
            inner.update = None;
            self.publish(&mut inner);
        }
        match Self::deadline(Duration::from_secs(30), operation).await {
            Ok(mut update) => {
                if let Some(update) = &mut update {
                    if !transport_allowed(&update.download_url, cfg!(debug_assertions)) {
                        return Ok(self.check_failed("Update downloads require HTTPS; development permits HTTP only on localhost.".into()));
                    }
                    update.timeout = Some(Duration::from_secs(600));
                }
                let mut inner = self.inner.lock().unwrap();
                inner.status.phase = if update.is_some() {
                    Phase::Available
                } else {
                    Phase::Current
                };
                inner.status.version = update.as_ref().map(|u| u.version.clone());
                inner.status.notes = update.as_ref().and_then(|u| u.body.clone());
                inner.update = update;
                if automatic
                    && inner.status.version.is_some()
                    && inner.status.version != inner.history.skipped_version
                {
                    inner.status.prompt_version = inner.status.version.clone();
                }
                let status = self.checked(&mut inner);
                let remind = status.prompt_version.is_some();
                drop(inner);
                // Publish first: a newly created webview restores the notice from its snapshot.
                if remind {
                    if let Err(error) = (self.foreground)() {
                        let mut inner = self.inner.lock().unwrap();
                        inner.status.error = Some(format!("Could not show update notice: {error}"));
                        return Ok(self.publish(&mut inner));
                    }
                }
                Ok(status)
            }
            Err(error) => Ok(self.check_failed(error)),
        }
    }
    /// One native operation owns the entire download, install and restart sequence.
    /// The work gate is acquired before downloading and released only on failure.
    pub async fn apply(
        &self,
        dirty: bool,
        acquire: impl FnOnce() -> Result<(), String>,
        release: impl FnOnce(),
        install: impl FnOnce(
            Update,
            Vec<u8>,
        )
            -> std::pin::Pin<Box<dyn Future<Output = Result<(), String>> + Send>>,
        restart: impl FnOnce() -> Result<(), String>,
    ) -> Result<Status, String> {
        let update = {
            let mut inner = self.inner.lock().unwrap();
            if dirty {
                return Err("Save your changes before installing an update.".into());
            }
            if let Some(reason) = &inner.status.install_disabled {
                return Err(reason.clone());
            }
            if matches!(
                inner.status.phase,
                Phase::Checking | Phase::Downloading | Phase::Installing
            ) || (inner.status.phase == Phase::Installed && inner.status.error.is_none())
            {
                return Err("An update operation is already active.".into());
            }
            // A failed restart retries only the restart, never installation.
            let update = if inner.status.phase == Phase::Installed {
                None
            } else {
                Some(
                    inner
                        .update
                        .clone()
                        .ok_or("Check for an available update first.")?,
                )
            };
            acquire()?;
            inner.status.prompt_version = None;
            inner.status.error = None;
            if update.is_some() {
                inner.status.phase = Phase::Downloading;
                inner.status.downloaded = 0;
                inner.status.total = None;
            }
            self.publish(&mut inner);
            update
        };
        if let Some(update) = update {
            let downloaded = Self::deadline(Duration::from_secs(600), async {
                update
                    .download(
                        |chunk, total| {
                            let mut inner = self.inner.lock().unwrap();
                            inner.status.downloaded += chunk as u64;
                            inner.status.total = total;
                            self.publish(&mut inner);
                        },
                        || {},
                    )
                    .await
                    .map_err(update_error)
            })
            .await;
            let bytes = match downloaded {
                Ok(bytes) => bytes,
                Err(error) => {
                    release();
                    return Ok(self.failed(error));
                }
            };
            {
                let mut inner = self.inner.lock().unwrap();
                inner.status.phase = Phase::Installing;
                self.publish(&mut inner);
            }
            // Do not cancel or time out an installer that may already be replacing files.
            if let Err(error) = install(update, bytes).await {
                release();
                return Ok(self.failed(format!("Installation failed: {error}")));
            }
            let mut inner = self.inner.lock().unwrap();
            inner.status.phase = Phase::Installed;
            self.publish(&mut inner);
        }
        if let Err(error) = restart() {
            release();
            let mut inner = self.inner.lock().unwrap();
            inner.status.error = Some(format!(
                "Restart failed: {error}. Retry the update to restart."
            ));
            return Ok(self.publish(&mut inner));
        }
        Ok(self.status())
    }
}
impl Status {
    pub fn applying(&self) -> bool {
        matches!(self.phase, Phase::Downloading | Phase::Installing)
            || (self.phase == Phase::Installed && self.error.is_none())
    }
    pub fn blocks_exit(&self, code: Option<i32>) -> bool {
        self.applying()
            && !(self.phase == Phase::Installed && code == Some(tauri::RESTART_EXIT_CODE))
    }
}

fn update_error(error: tauri_plugin_updater::Error) -> String {
    use tauri_plugin_updater::Error::*;
    match error {
        ReleaseNotFound => "The update server did not return a release feed. Check that latest.json is being served at the configured endpoint.".into(),
        Reqwest(ref e) if e.is_connect() => "Cannot reach the update server. Check your connection and retry.".into(),
        Reqwest(ref e) if e.is_timeout() => "Update request timed out. Check your connection and retry.".into(),
        Minisign(_) | Base64(_) | SignatureUtf8(_) | SignedVersionMismatch { .. } | MissingSignedVersion => "Update signature verification failed. Nothing was installed; retry or contact the release publisher.".into(),
        TargetNotFound(_) | TargetsNotFound(_) => "This release has no update for your operating system and architecture.".into(),
        Reqwest(ref e) if e.is_decode() => "The update feed is malformed. Retry after the publisher fixes latest.json.".into(),
        Serialization(_) => "The update feed is malformed. Retry after the publisher fixes latest.json.".into(),
        other => format!("Update failed: {other}"),
    }
}
fn transport_allowed(url: &reqwest::Url, development: bool) -> bool {
    url.scheme() == "https"
        || (development
            && url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
}
fn transport_builder<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    development: bool,
) -> tauri_plugin_updater::UpdaterBuilder {
    app.updater_builder()
        .timeout(Duration::from_secs(30))
        .configure_client(move |client| {
            client
                .https_only(!development)
                .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                    if attempt.previous().len() >= 10 {
                        attempt.error("Too many update redirects")
                    } else if transport_allowed(attempt.url(), development) {
                        attempt.follow()
                    } else {
                        attempt.error(
                            "Updates require HTTPS; development permits HTTP only on localhost",
                        )
                    }
                }))
        })
}
fn configured_builder<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Result<tauri_plugin_updater::UpdaterBuilder, String> {
    let builder = transport_builder(app, cfg!(debug_assertions));
    #[cfg(debug_assertions)]
    {
        builder
            .no_proxy()
            .endpoints(vec!["http://127.0.0.1:8080/latest.json"
                .parse()
                .expect("static local update URL")])
            .map_err(update_error)
    }
    #[cfg(not(debug_assertions))]
    {
        Ok(builder)
    }
}
pub fn install_disabled() -> Option<String> {
    installation_block_reason(
        std::env::consts::OS,
        std::env::current_exe().ok().as_deref(),
        std::env::var_os("APPIMAGE").is_some(),
    )
}

fn installation_block_reason(
    platform: &str,
    executable: Option<&std::path::Path>,
    appimage: bool,
) -> Option<String> {
    if platform == "linux" && !appimage {
        Some("In-app installation currently supports Linux AppImage. Update DEB installations through your package manager.".into())
    } else if platform == "macos"
        && !executable.is_some_and(|path| {
            path.parent().is_some_and(|macos| {
                macos.ends_with("Contents/MacOS")
                    && macos
                        .parent()
                        .and_then(std::path::Path::parent)
                        .and_then(std::path::Path::extension)
                        .is_some_and(|extension| extension == "app")
            })
        })
    {
        Some("Launch OpenStreamDeck from its macOS app bundle before applying updates. cargo tauri dev creates this bundle automatically.".into())
    } else {
        None
    }
}

#[tauri::command]
pub fn update_status(service: tauri::State<Arc<Service>>) -> Status {
    service.status()
}
async fn query_update(app: &tauri::AppHandle) -> Result<Option<Update>, String> {
    let cleanup = app.clone();
    let updater = configured_builder(app)?
        .on_before_exit(move || cleanup.cleanup_before_exit())
        .restart_after_install(true)
        .build()
        .map_err(update_error)?;
    updater.check().await.map_err(update_error)
}

async fn run_scheduler<F, Fut>(service: Arc<Service>, operation: F)
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<Option<Update>, String>>,
{
    loop {
        let delay = service.automatic_delay();
        if !delay.is_zero() {
            tokio::time::sleep(delay).await;
            continue;
        }
        // Uses the same lock, deadlines, history and signature configuration as manual checks.
        let _ = service.check_inner(operation(), true).await;
    }
}

#[tauri::command]
pub async fn check_updates(
    app: tauri::AppHandle,
    service: tauri::State<'_, Arc<Service>>,
) -> Result<Status, String> {
    service.check(query_update(&app)).await
}
#[tauri::command]
pub fn skip_update(service: tauri::State<Arc<Service>>, version: String) -> Result<Status, String> {
    service.skip(&version)
}
#[tauri::command]
pub async fn install_update(
    app: tauri::AppHandle,
    service: tauri::State<'_, Arc<Service>>,
    dirty: bool,
) -> Result<Status, String> {
    let service = service.inner().clone();
    // Detach the operation from the webview's IPC lifetime. Closing/reloading the
    // webview cannot stop an installation after the user has started it.
    tauri::async_runtime::spawn(async move {
        service
            .apply(
                dirty,
                || app.state::<crate::AppState>().begin_update_install(),
                || {
                    app.state::<crate::AppState>()
                        .update_installing
                        .store(false, std::sync::atomic::Ordering::SeqCst)
                },
                |update, bytes| {
                    Box::pin(async move {
                        tokio::task::spawn_blocking(move || {
                            update.install(&bytes).map_err(update_error)
                        })
                        .await
                        .map_err(|e| format!("Installer task failed: {e}"))?
                    })
                },
                || {
                    app.request_restart();
                    Ok(())
                },
            )
            .await
    })
    .await
    .map_err(|e| format!("Update task failed: {e}"))?
}

pub fn setup(app: &tauri::AppHandle) {
    let emit_app = app.clone();
    let mut service = Service::new(
        app.package_info().version.to_string(),
        install_disabled(),
        move |status| {
            let _ = emit_app.emit("update_status", status);
        },
    )
    .with_history(
        app.state::<crate::AppState>()
            .dir
            .join("updater-state.json"),
    );
    let foreground_app = app.clone();
    service.foreground = Box::new(move || {
        crate::navigation::foreground(&crate::navigation::NativeWindow(&foreground_app))
    });
    let service = Arc::new(service);
    app.manage(service);
}
pub fn start_scheduler(app: &tauri::AppHandle) {
    let service = app.state::<Arc<Service>>().inner().clone();
    let app = app.clone();
    tauri::async_runtime::spawn(run_scheduler(service, move || {
        let app = app.clone();
        async move { query_update(&app).await }
    }));
}
