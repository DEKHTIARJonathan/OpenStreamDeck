pub mod actions;
pub mod agents;
pub mod ai;
pub mod applications;
pub mod artwork;
pub mod codex_adapter;
pub mod device;
pub mod diagnostics;
pub mod engine;
pub mod focus;
pub mod foreground;
pub mod model;
pub mod navigation;
mod onboarding;
pub mod page_context;
#[cfg(windows)]
pub mod process_tree;
pub mod protocol;
mod release_notes;
mod settings;
pub mod store;
mod updates;

use model::*;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::SyncSender,
        Arc, Mutex,
    },
};
use tauri::{Emitter, Manager};

pub struct AppState {
    pub dir: PathBuf,
    pub config: Mutex<Config>,
    pub device: Mutex<DeviceStatus>,
    pub tx: SyncSender<device::DeviceCommand>,
    pub active: Mutex<HashMap<String, String>>,
    pub cancellations: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pub cycles: Mutex<HashMap<String, usize>>,
    pub agents: Mutex<Vec<AgentSession>>,
    pub hook_agents: Mutex<HashMap<String, AgentSession>>,
    pub adapter_errors: Mutex<Vec<String>>,
    pub selected_agent: Mutex<usize>,
    pub agent_frame: Mutex<String>,
    pub navigation: Mutex<navigation::Navigation>,
    pub settings_error: Mutex<Option<String>>,
    pub update_installing: AtomicBool,
    pub page_context: Mutex<page_context::PageContext>,
    pub quitting: AtomicBool,
}
impl AppState {
    fn new(dir: PathBuf, config: Config, tx: SyncSender<device::DeviceCommand>) -> Self {
        Self {
            dir,
            page_context: Mutex::new(page_context::PageContext::new(&config)),
            quitting: AtomicBool::new(false),
            config: Mutex::new(config),
            device: Mutex::new(DeviceStatus {
                message: "Not connected".into(),
                ..Default::default()
            }),
            tx,
            active: Mutex::new(HashMap::new()),
            cancellations: Mutex::new(HashMap::new()),
            cycles: Mutex::new(HashMap::new()),
            agents: Mutex::new(vec![]),
            hook_agents: Mutex::new(HashMap::new()),
            adapter_errors: Mutex::new(vec![]),
            selected_agent: Mutex::new(0),
            agent_frame: Mutex::new(String::new()),
            navigation: Mutex::new(navigation::Navigation::default()),
            settings_error: Mutex::new(None),
            update_installing: AtomicBool::new(false),
        }
    }

    pub(crate) fn begin_update_install(&self) -> Result<(), String> {
        let active = self.active.lock().unwrap();
        // The binding map retains only the latest overlapping run. The
        // cancellation registry retains every run until its future completes.
        // Match action registration's active -> cancellations lock order.
        if !active.is_empty() || !self.cancellations.lock().unwrap().is_empty() {
            return Err(
                "Wait for running actions and workflows to finish before installing.".into(),
            );
        }
        self.update_installing.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn save_configuration(
        &self,
        config: Config,
        login: &impl settings::LoginRegistration,
    ) -> Result<bool, String> {
        // Serialize persistence with installation, including saves queued before
        // the frontend receives the update progress event.
        let _active = self.active.lock().unwrap();
        if self.update_installing.load(Ordering::SeqCst) {
            return Err(
                "An update is in progress. Changes cannot be saved until it finishes.".into(),
            );
        }
        let mut current = self.config.lock().unwrap();
        let brightness_changed = current.brightness != config.brightness;
        settings::save(&self.dir, &mut current, config, login)?;
        *self.settings_error.lock().unwrap() = None;
        self.page_context.lock().unwrap().configure(&current);
        Ok(brightness_changed)
    }

    fn finish_setup(
        &self,
        setup: &mut onboarding::Setup,
        login: &impl settings::LoginRegistration,
        enable_login: bool,
    ) -> Result<(), String> {
        // Use the same gate and lock order as configuration saves/installations.
        let _active = self.active.lock().unwrap();
        if self.update_installing.load(Ordering::SeqCst) {
            return Err("An update is in progress. Finish setup after it completes.".into());
        }
        onboarding::finish(
            &self.dir,
            setup,
            &mut self.config.lock().unwrap(),
            login,
            enable_login,
        )
    }

    pub fn send(&self, command: device::DeviceCommand) -> Result<(), String> {
        if command.requires_connection() && !self.device.lock().unwrap().connected {
            return Err("N3 is not connected".into());
        }
        self.tx
            .try_send(command)
            .map_err(|e| format!("Device queue: {e}"))
    }
}
#[derive(serde::Serialize)]
struct Snapshot {
    onboarding: bool,
    platform: &'static str,
    navigation: navigation::Navigation,
    config: Config,
    device: DeviceStatus,
    agents: Vec<AgentSession>,
    adapter_errors: Vec<String>,
    data_dir: String,
    settings_error: Option<String>,
    update: updates::Status,
    page_context: page_context::PageContext,
}
#[tauri::command]
fn snapshot(app: tauri::AppHandle, state: tauri::State<AppState>) -> Snapshot {
    // Do not keep temporary guards alive across the entire struct expression:
    // collecting independent fields must not nest config/device/agent locks.
    let navigation = state.navigation.lock().unwrap().clone();
    let config = state.config.lock().unwrap().clone();
    let device = state.device.lock().unwrap().clone();
    let agents = state.agents.lock().unwrap().clone();
    let adapter_errors = state.adapter_errors.lock().unwrap().clone();
    let settings_error = state.settings_error.lock().unwrap().clone();
    let page_context = state.page_context.lock().unwrap().clone();
    Snapshot {
        onboarding: app.state::<OnboardingState>().0.lock().unwrap().pending(),
        update: app.state::<Arc<updates::Service>>().status(),
        platform: std::env::consts::OS,
        navigation,
        config,
        device,
        agents,
        adapter_errors,
        data_dir: state.dir.display().to_string(),
        settings_error,
        page_context,
    }
}
struct OnboardingState(Mutex<onboarding::State>);

#[tauri::command]
fn onboarding_status(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
) -> Result<onboarding::Status, String> {
    let service = app.state::<OnboardingState>();
    let mut setup = service.0.lock().unwrap();
    Ok(onboarding::status(
        setup.setup(&state.dir)?,
        &settings::NativeLogin(&app),
    ))
}

#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    onboarding::request_access(std::env::consts::OS, &onboarding::NativePermission)
}

#[tauri::command]
fn finish_onboarding(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    enable_login: bool,
) -> Result<(), String> {
    let service = app.state::<OnboardingState>();
    let mut setup = service.0.lock().unwrap();
    let result = state.finish_setup(
        setup.setup(&state.dir)?,
        &settings::NativeLogin(&app),
        enable_login,
    );
    drop(setup);
    let config = state.config.lock().unwrap().clone();
    let _ = app.emit("config", config);
    result
}

#[tauri::command]
fn set_view(state: tauri::State<AppState>, view: navigation::View) -> navigation::Navigation {
    navigation::select(&state, view)
}
#[tauri::command]
fn save_config(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    config: Config,
) -> Result<(), String> {
    let brightness_changed =
        state.save_configuration(config.clone(), &settings::NativeLogin(&app))?;
    page_context::publish(&app);
    // Persistence succeeded. A busy device queue must not turn this into a
    // misleading failed save or hide the authoritative config update.
    let _ = app.emit("config", &config);
    if !state.device.lock().unwrap().connected {
        return Ok(());
    }
    let queued = if brightness_changed {
        state.send(device::DeviceCommand::Brightness(config.brightness))
    } else {
        Ok(())
    };
    if let Err(error) = queued {
        let _ = app.emit(
            "action_error",
            format!("Config saved, but device update failed: {error}"),
        );
    }
    Ok(())
}
#[tauri::command]
fn connect_device(state: tauri::State<AppState>, connect: bool) -> Result<(), String> {
    state.send(device::DeviceCommand::Connect(connect))
}
#[tauri::command]
async fn list_devices() -> Result<Vec<device::Candidate>, String> {
    tokio::task::spawn_blocking(device::enumerate)
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
fn sleep_device(state: tauri::State<AppState>, sleep: bool) -> Result<(), String> {
    state.send(device::DeviceCommand::Sleep(sleep))
}
#[tauri::command]
async fn run_action(app: tauri::AppHandle, action: Action, binding: String) -> String {
    actions::start(app, action, binding).await
}
#[tauri::command]
fn cancel_run(state: tauri::State<AppState>, id: String) -> Result<(), String> {
    if let Some(c) = state.cancellations.lock().unwrap().get(&id) {
        c.store(true, Ordering::Relaxed);
        Ok(())
    } else {
        Err("Run is no longer active".into())
    }
}
#[tauri::command]
async fn set_provider_key(
    state: tauri::State<'_, AppState>,
    provider: String,
    key: String,
) -> Result<(), String> {
    let account = {
        let w = state.config.lock().unwrap();
        let saved = w
            .providers
            .iter()
            .find(|p| p.id == provider)
            .ok_or("Save the provider configuration first")?;
        ai::credential_account(saved)
    };
    tokio::task::spawn_blocking(move || {
        let entry = keyring::Entry::new("OpenStreamDeck", &account).map_err(|e| e.to_string())?;
        if key.is_empty() {
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        } else {
            entry.set_password(&key).map_err(|e| e.to_string())
        }
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn agent_focus(state: tauri::State<'_, AppState>, id: String) -> Result<(), String> {
    let session = state
        .agents
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or("Session not found")?;
    agents::focus(&session).await
}
#[tauri::command]
async fn agent_prompt(
    state: tauri::State<'_, AppState>,
    id: String,
    text: String,
) -> Result<(), String> {
    let session = state
        .agents
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.id == id)
        .cloned()
        .ok_or("Session not found")?;
    agents::prompt(&session, &text).await
}
#[tauri::command]
fn show_agents(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    enabled: bool,
) -> Result<(), String> {
    state.page_context.lock().unwrap().set_overlay(enabled);
    page_context::publish(&app);
    state.agent_frame.lock().unwrap().clear();
    if enabled {
        let sessions = state.agents.lock().unwrap().clone();
        agents::display(&app, &sessions)
    }
    Ok(())
}
#[tauri::command]
fn import_config(value: serde_json::Value) -> Result<Config, String> {
    let config: Config = serde_json::from_value(value).map_err(|e| e.to_string())?;
    config.validate()?;
    Ok(config)
}
#[tauri::command]
async fn read_config_file(path: String) -> Result<Config, String> {
    use tokio::io::AsyncReadExt;
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;
    let mut data = Vec::new();
    file.take(20_000_001)
        .read_to_end(&mut data)
        .await
        .map_err(|e| e.to_string())?;
    if data.len() > 20_000_000 {
        return Err("Config exceeds 20 MB".into());
    }
    Config::decode(&data)
}
#[tauri::command]
async fn write_config_file(path: String, config: Config) -> Result<(), String> {
    config.validate()?;
    let data = serde_json::to_vec_pretty(&config).map_err(|e| e.to_string())?;
    tokio::fs::write(path, data)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn render_keys(
    actions: Vec<Action>,
    size: u32,
    rotation: u16,
) -> Result<Vec<String>, String> {
    if actions.len() > 6 {
        return Err("At most six LCD frames per request".into());
    }
    tokio::task::spawn_blocking(move || {
        actions
            .iter()
            .map(|a| artwork::render(a, size, rotation).map(|b| artwork::data_url(&b)))
            .collect()
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn list_applications() -> Result<Vec<applications::Application>, String> {
    tokio::task::spawn_blocking(applications::list)
        .await
        .map_err(|e| e.to_string())
}
#[tauri::command]
async fn application_icon(path: String) -> Result<String, String> {
    applications::icon(&path).await
}

#[tauri::command]
async fn application_identity(path: String) -> Result<ApplicationAssociation, String> {
    applications::association(&path).await
}

#[tauri::command]
fn select_page(app: tauri::AppHandle, page: String) -> Result<(), String> {
    page_context::select(&app, &page)
}

#[tauri::command]
fn resume_automatic(app: tauri::AppHandle, state: tauri::State<AppState>) {
    let config = state.config.lock().unwrap();
    state.page_context.lock().unwrap().resume(&config);
    drop(config);
    page_context::publish(&app);
}

fn show_window(app: &tauri::AppHandle) {
    if let Err(error) = navigation::foreground(&navigation::NativeWindow(app)) {
        eprintln!("{error}");
        let _ = app.emit("action_error", error);
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if args.iter().any(|a| a == "--hardware-smoke") {
                if let Err(error) = app
                    .state::<AppState>()
                    .send(device::DeviceCommand::HardwareSmoke)
                {
                    let _ = app.emit("action_error", error);
                }
                return;
            }
            if args
                .iter()
                .any(|a| a == "--background" || a == "--from-autostart")
            {
                return;
            }
            if args.iter().any(|a| a == "--close-window") {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.close();
                }
            } else {
                show_window(app);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_updater::Builder::new()
                .pubkey(env!("TAURI_SIGNING_PUBLIC_KEY"))
                .build(),
        )
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("OpenStreamDeck")
                .arg("--from-autostart")
                .build(),
        )
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            app.manage(OnboardingState(Mutex::new(onboarding::State::new(&dir))));
            let (config, settings_error) =
                settings::load(&dir, &settings::NativeLogin(app.handle()))
                    .map_err(std::io::Error::other)?;
            let (tx, rx) = std::sync::mpsc::sync_channel(64);
            app.manage(AppState::new(dir, config, tx));
            updates::setup(app.handle());
            *app.state::<AppState>().settings_error.lock().unwrap() = settings_error;
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("foreground-app".into())
                .spawn(move || page_context::monitor(handle))?;
            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("n3-hid".into())
                .spawn(move || device::worker(handle, rx))?;
            tauri::async_runtime::spawn(agents::monitor(app.handle().clone()));
            tauri::async_runtime::spawn(agents::hook_server(app.handle().clone()));
            #[cfg(target_os = "macos")]
            {
                use tauri::menu::{Menu, MenuItem, PredefinedMenuItem as Item, Submenu};
                // Predefined Quit requests code=None, which also means last-window
                // close. An explicit exit(0) keeps Cmd+Q distinct from tray mode.
                let quit = MenuItem::with_id(
                    app,
                    "app-quit",
                    "Quit OpenStreamDeck",
                    true,
                    Some("CmdOrCtrl+Q"),
                )?;
                let app_menu = Submenu::with_items(
                    app,
                    "OpenStreamDeck",
                    true,
                    &[
                        &Item::about(app, None, None)?,
                        &Item::separator(app)?,
                        &quit,
                    ],
                )?;
                let edit = Submenu::with_items(
                    app,
                    "Edit",
                    true,
                    &[
                        &Item::undo(app, None)?,
                        &Item::redo(app, None)?,
                        &Item::separator(app)?,
                        &Item::cut(app, None)?,
                        &Item::copy(app, None)?,
                        &Item::paste(app, None)?,
                        &Item::select_all(app, None)?,
                    ],
                )?;
                app.set_menu(Menu::with_items(app, &[&app_menu, &edit])?)?;
                app.on_menu_event(|app, event| {
                    if event.id.as_ref() == "app-quit" {
                        app.exit(0);
                    }
                });
            }
            let show = tauri::menu::MenuItem::with_id(
                app,
                "show",
                "Open OpenStreamDeck",
                true,
                None::<&str>,
            )?;
            let quit = tauri::menu::MenuItem::with_id(
                app,
                "quit",
                "Quit OpenStreamDeck",
                true,
                None::<&str>,
            )?;
            let menu = tauri::menu::Menu::with_items(app, &[&show, &quit])?;
            let mut tray = tauri::tray::TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("OpenStreamDeck · N3")
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();
                    if let Err(error) =
                        navigation::tray_click(&event, &navigation::NativeWindow(app))
                    {
                        eprintln!("{error}");
                        let _ = app.emit("action_error", error);
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            if std::env::args().any(|a| a == "--hardware-smoke") {
                let app = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    for _ in 0..80 {
                        if app.state::<AppState>().device.lock().unwrap().connected {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    if let Err(error) = app
                        .state::<AppState>()
                        .send(device::DeviceCommand::HardwareSmoke)
                    {
                        let _ = app.emit("action_error", error);
                    }
                });
            }
            if !settings::start_in_background(
                app.state::<AppState>().config.lock().unwrap().menu_bar_only,
                &std::env::args().collect::<Vec<_>>(),
            ) {
                show_window(app.handle());
            } else {
                #[cfg(target_os = "macos")]
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }
            updates::start_scheduler(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            release_notes::open_release_link,
            updates::update_status,
            updates::check_updates,
            updates::skip_update,
            updates::install_update,
            snapshot,
            onboarding_status,
            open_accessibility_settings,
            finish_onboarding,
            set_view,
            render_keys,
            list_applications,
            application_icon,
            application_identity,
            select_page,
            resume_automatic,
            save_config,
            connect_device,
            list_devices,
            sleep_device,
            run_action,
            cancel_run,
            set_provider_key,
            agent_focus,
            agent_prompt,
            show_agents,
            import_config,
            read_config_file,
            write_config_file
        ])
        .build(tauri::generate_context!())
        .expect("OpenStreamDeck startup failed")
        .run(|app, event| match event {
            tauri::RunEvent::Exit => {
                app.state::<AppState>()
                    .quitting
                    .store(true, Ordering::Relaxed);
            }
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                if app
                    .state::<Arc<updates::Service>>()
                    .status()
                    .blocks_exit(code)
                {
                    api.prevent_exit();
                    let _ = app.emit(
                        "action_error",
                        "The update is in progress. OpenStreamDeck will restart automatically.",
                    );
                } else if settings::keep_running_after_exit(
                    app.state::<AppState>().config.lock().unwrap().menu_bar_only,
                    code,
                ) {
                    api.prevent_exit();
                }
            }
            tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::CloseRequested { api, .. },
                ..
            } if app.state::<Arc<updates::Service>>().status().applying() => {
                api.prevent_close();
            }
            tauri::RunEvent::WindowEvent {
                label,
                event: tauri::WindowEvent::Destroyed,
                ..
            } if label == "main" => {
                let menu_bar_only = app.state::<AppState>().config.lock().unwrap().menu_bar_only;
                if let Err(error) =
                    settings::window_closed(menu_bar_only, &settings::NativeClose(app))
                {
                    eprintln!("{error}");
                    *app.state::<AppState>().settings_error.lock().unwrap() = Some(error);
                }
            }
            _ => {}
        });
}

#[cfg(test)]
mod config_file_tests {
    use super::*;

    #[test]
    fn imports_require_the_current_complete_schema() {
        let current = serde_json::to_value(Config::default()).unwrap();
        assert!(import_config(current.clone()).is_ok());
        for version in [0, 1, 3] {
            let mut unsupported = current.clone();
            unsupported["version"] = version.into();
            assert_eq!(
                import_config(unsupported).unwrap_err(),
                "Unsupported config version"
            );
        }
        for missing in ["version", "pages", "active_page", "providers", "workflows"] {
            let mut incomplete = current.clone();
            incomplete.as_object_mut().unwrap().remove(missing);
            assert!(import_config(incomplete).is_err(), "{missing}");
        }
        let mut extra = current;
        extra["unexpected"] = true.into();
        assert!(import_config(extra).is_err());
        assert!(import_config(serde_json::json!({"version": 2})).is_err());
    }

    #[tokio::test]
    async fn config_file_round_trip_preserves_unknown_extension_actions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json").to_str().unwrap().to_string();
        let mut value = serde_json::to_value(Config::default()).unwrap();
        let action = &mut value["pages"][0]["bindings"]["lcd1"];
        action["kind"] = "future-extension".into();
        action["value"] = "opaque configuration".into();
        action["options"] = serde_json::json!({"extension-setting": "preserve"});
        let config = import_config(value.clone()).unwrap();
        write_config_file(path.clone(), config).await.unwrap();
        let loaded = read_config_file(path.clone()).await.unwrap();
        assert_eq!(serde_json::to_value(loaded).unwrap(), value);

        value["version"] = 1.into();
        let unsupported = serde_json::to_vec(&value).unwrap();
        std::fs::write(&path, &unsupported).unwrap();
        assert_eq!(
            read_config_file(path.clone()).await.unwrap_err(),
            "Unsupported config version"
        );
        assert_eq!(std::fs::read(path).unwrap(), unsupported);
    }
}

#[cfg(test)]
mod queue_tests {
    use super::*;
    use device::DeviceCommand;
    #[test]
    fn device_queue_rejects_offline_full_and_closed_without_touching_hardware() {
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let state = AppState::new(PathBuf::new(), Config::default(), tx);
        for command in [
            DeviceCommand::Brightness(50),
            DeviceCommand::Sleep(true),
            DeviceCommand::ToggleSleep,
            DeviceCommand::Redraw,
            DeviceCommand::AgentFrame(0, vec![], vec![], String::new()),
            DeviceCommand::Raw(vec![0]),
        ] {
            assert_eq!(state.send(command).unwrap_err(), "N3 is not connected");
        }
        state.send(DeviceCommand::Connect(true)).unwrap();
        assert!(matches!(rx.recv().unwrap(), DeviceCommand::Connect(true)));
        state.send(DeviceCommand::HardwareSmoke).unwrap();
        assert!(matches!(rx.recv().unwrap(), DeviceCommand::HardwareSmoke));
        state.device.lock().unwrap().connected = true;
        state.send(DeviceCommand::Brightness(50)).unwrap();
        assert!(state
            .send(DeviceCommand::Sleep(false))
            .unwrap_err()
            .contains("Device queue"));
        drop(rx);
        assert!(state
            .send(DeviceCommand::Redraw)
            .unwrap_err()
            .contains("Device queue"));
    }
}
