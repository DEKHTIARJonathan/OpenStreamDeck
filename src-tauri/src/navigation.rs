//! OpenStreamDeck window navigation, separate from agent focus and LCD display modes.
use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub enum View {
    #[default]
    Deck,
    #[serde(rename = "AI workflows")]
    Workflows,
    Agents,
    Settings,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq, Eq)]
pub struct Navigation {
    pub view: View,
    pub revision: u64,
}

pub fn select(state: &AppState, view: View) -> Navigation {
    let mut navigation = state.navigation.lock().unwrap();
    navigation.view = view;
    navigation.revision += 1;
    navigation.clone()
}

// Tests replace only the OS/window boundary, exercising the same sequencing
// used by actions, tray reopening and single-instance activation.
pub(crate) trait WindowHost {
    fn activate(&self) -> Result<(), String>;
    fn ensure_window(&self) -> Result<(), String>;
    fn unminimize(&self) -> Result<(), String>;
    fn show(&self) -> Result<(), String>;
    fn focus(&self) -> Result<(), String>;
    fn navigate(&self, navigation: &Navigation) -> Result<(), String>;
}

pub(crate) fn foreground(host: &impl WindowHost) -> Result<(), String> {
    host.activate()?;
    host.ensure_window()?;
    host.unminimize()?;
    host.show()?;
    host.focus()
}

pub(crate) fn tray_click(
    event: &tauri::tray::TrayIconEvent,
    host: &impl WindowHost,
) -> Result<(), String> {
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
    if matches!(
        event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    ) {
        foreground(host)?;
    }
    Ok(())
}

pub(crate) fn dashboard(state: &AppState, host: &impl WindowHost) -> Result<(), String> {
    // Store before creating a webview: its startup snapshot must recover the
    // requested tab even if the navigation event arrives before JS is listening.
    let navigation = select(state, View::Agents);
    foreground(host)?;
    host.navigate(&navigation)
}

pub(crate) struct NativeWindow<'a>(pub &'a tauri::AppHandle);
impl NativeWindow<'_> {
    fn window(&self) -> Result<tauri::WebviewWindow, String> {
        self.0
            .get_webview_window("main")
            .ok_or_else(|| "OpenStreamDeck window is unavailable".into())
    }
}
impl WindowHost for NativeWindow<'_> {
    fn activate(&self) -> Result<(), String> {
        #[cfg(target_os = "macos")]
        self.0
            .set_activation_policy(tauri::ActivationPolicy::Regular)
            .map_err(|error| format!("Could not activate OpenStreamDeck: {error}"))?;
        #[cfg(target_os = "macos")]
        self.0
            .show()
            .map_err(|error| format!("Could not unhide OpenStreamDeck: {error}"))?;
        Ok(())
    }

    fn ensure_window(&self) -> Result<(), String> {
        if self.0.get_webview_window("main").is_none() {
            let config = self
                .0
                .config()
                .app
                .windows
                .first()
                .ok_or("OpenStreamDeck window configuration is missing")?;
            tauri::WebviewWindowBuilder::from_config(self.0, config)
                .and_then(|builder| builder.build())
                .map_err(|error| format!("Could not create OpenStreamDeck window: {error}"))?;
        }
        Ok(())
    }

    fn unminimize(&self) -> Result<(), String> {
        self.window()?
            .unminimize()
            .map_err(|error| format!("Could not restore OpenStreamDeck window: {error}"))
    }

    fn show(&self) -> Result<(), String> {
        self.window()?
            .show()
            .map_err(|error| format!("Could not show OpenStreamDeck window: {error}"))
    }

    fn focus(&self) -> Result<(), String> {
        self.window()?
            .set_focus()
            .map_err(|error| format!("Could not focus OpenStreamDeck window: {error}"))
    }

    fn navigate(&self, navigation: &Navigation) -> Result<(), String> {
        self.0
            .emit_to("main", "navigate", navigation)
            .map_err(|error| format!("Could not navigate OpenStreamDeck: {error}"))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn main_window_preserves_html_page_drag_and_drop() {
        // Startup and tray recreation both use this production window config.
        // Tauri's native file-drop handler consumes drops before HTML tabs see
        // them, including WKWebView's performDragOperation on macOS.
        let config: tauri::utils::config::Config =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let main = config
            .app
            .windows
            .iter()
            .find(|window| window.label == "main")
            .expect("main window configuration");
        assert!(
            !main.drag_drop_enabled,
            "Native file-drop interception must be disabled for HTML page reordering"
        );
    }

    use super::*;
    use crate::model::Config;
    use std::{
        cell::{Cell, RefCell},
        path::PathBuf,
        sync::mpsc::sync_channel,
    };

    struct Window<'a> {
        state: &'a AppState,
        exists: Cell<bool>,
        calls: RefCell<Vec<&'static str>>,
        fail: Option<&'static str>,
    }
    impl Window<'_> {
        fn record(&self, operation: &'static str) -> Result<(), String> {
            self.calls.borrow_mut().push(operation);
            if self.fail == Some(operation) {
                Err(format!("Fixture: {operation} failed"))
            } else {
                Ok(())
            }
        }
    }
    impl WindowHost for Window<'_> {
        fn activate(&self) -> Result<(), String> {
            self.record("activate")
        }
        fn ensure_window(&self) -> Result<(), String> {
            assert_eq!(self.state.navigation.lock().unwrap().view, View::Agents);
            self.record(if self.exists.replace(true) {
                "reuse"
            } else {
                "create"
            })
        }
        fn unminimize(&self) -> Result<(), String> {
            self.record("unminimize")
        }
        fn show(&self) -> Result<(), String> {
            self.record("show")
        }
        fn focus(&self) -> Result<(), String> {
            self.record("focus")
        }
        fn navigate(&self, request: &Navigation) -> Result<(), String> {
            assert_eq!(request, &*self.state.navigation.lock().unwrap());
            self.record("navigate")
        }
    }

    fn click(
        button: tauri::tray::MouseButton,
        button_state: tauri::tray::MouseButtonState,
    ) -> tauri::tray::TrayIconEvent {
        tauri::tray::TrayIconEvent::Click {
            id: "test-tray".into(),
            position: tauri::PhysicalPosition::new(10.0, 10.0),
            rect: tauri::Rect {
                position: tauri::PhysicalPosition::new(0, 0).into(),
                size: tauri::PhysicalSize::new(20, 20).into(),
            },
            button,
            button_state,
        }
    }

    #[test]
    fn left_tray_click_restores_or_recreates_window_without_changing_navigation() {
        use tauri::tray::{MouseButton, MouseButtonState};
        for exists in [false, true] {
            let (tx, rx) = sync_channel(1);
            let state = AppState::new(PathBuf::new(), Config::default(), tx);
            let navigation = select(&state, View::Agents);
            let host = Window {
                state: &state,
                exists: Cell::new(exists),
                calls: RefCell::default(),
                fail: None,
            };
            for button in [MouseButton::Right, MouseButton::Middle, MouseButton::Left] {
                tray_click(&click(button, MouseButtonState::Down), &host).unwrap();
                if button != MouseButton::Left {
                    tray_click(&click(button, MouseButtonState::Up), &host).unwrap();
                }
            }
            assert!(
                host.calls.borrow().is_empty(),
                "right-click must leave foregrounding to the menu"
            );
            tray_click(&click(MouseButton::Left, MouseButtonState::Up), &host).unwrap();
            assert_eq!(
                *host.calls.borrow(),
                [
                    "activate",
                    if exists { "reuse" } else { "create" },
                    "unminimize",
                    "show",
                    "focus"
                ]
            );
            assert_eq!(*state.navigation.lock().unwrap(), navigation);
            assert!(!state.page_context.lock().unwrap().overlay);
            assert!(rx.try_recv().is_err());
        }
    }

    #[test]
    fn tray_click_propagates_foreground_failures() {
        use tauri::tray::{MouseButton, MouseButtonState};
        for failure in ["activate", "create", "unminimize", "show", "focus"] {
            let (tx, _) = sync_channel(1);
            let state = AppState::new(PathBuf::new(), Config::default(), tx);
            select(&state, View::Agents);
            let host = Window {
                state: &state,
                exists: Cell::new(false),
                calls: RefCell::default(),
                fail: Some(failure),
            };
            assert_eq!(
                tray_click(&click(MouseButton::Left, MouseButtonState::Up), &host).unwrap_err(),
                format!("Fixture: {failure} failed")
            );
        }
    }

    #[test]
    fn dashboard_restores_or_creates_our_window_without_changing_lcd_frames_or_bindings() {
        for exists in [true, false] {
            let (tx, rx) = sync_channel(8);
            let state = AppState::new(PathBuf::new(), Config::default(), tx);
            let before = serde_json::to_value(&*state.config.lock().unwrap()).unwrap();
            let frames = || {
                let config = state.config.lock().unwrap();
                (1..=6)
                    .map(|i| {
                        crate::artwork::render(
                            &config.page().unwrap().bindings[&format!("lcd{i}")],
                            config.image_size,
                            config.rotation,
                        )
                        .unwrap()
                    })
                    .collect::<Vec<_>>()
            };
            let original = frames();
            let host = Window {
                state: &state,
                exists: Cell::new(exists),
                calls: RefCell::default(),
                fail: None,
            };
            for revision in 1..=2 {
                host.calls.borrow_mut().clear();
                dashboard(&state, &host).unwrap();
                assert_eq!(
                    *host.calls.borrow(),
                    vec![
                        "activate",
                        if exists || revision > 1 {
                            "reuse"
                        } else {
                            "create"
                        },
                        "unminimize",
                        "show",
                        "focus",
                        "navigate"
                    ]
                );
                assert_eq!(state.navigation.lock().unwrap().revision, revision);
                assert!(!state.page_context.lock().unwrap().overlay);
                assert!(
                    rx.try_recv().is_err(),
                    "dashboard must not send LCD commands"
                );
                assert_eq!(
                    serde_json::to_value(&*state.config.lock().unwrap()).unwrap(),
                    before
                );
                assert_eq!(frames(), original);
            }
        }
    }

    #[test]
    fn dashboard_surfaces_each_window_failure_and_retains_the_requested_tab() {
        let operations = [
            "activate",
            "create",
            "unminimize",
            "show",
            "focus",
            "navigate",
        ];
        for (index, operation) in operations.iter().enumerate() {
            let (tx, rx) = sync_channel(1);
            let state = AppState::new(PathBuf::new(), Config::default(), tx);
            let host = Window {
                state: &state,
                exists: Cell::new(false),
                calls: RefCell::default(),
                fail: Some(operation),
            };
            assert_eq!(
                dashboard(&state, &host).unwrap_err(),
                format!("Fixture: {operation} failed")
            );
            assert_eq!(*host.calls.borrow(), operations[..=index]);
            assert_eq!(state.navigation.lock().unwrap().view, View::Agents);
            assert!(rx.try_recv().is_err());
        }
    }

    #[test]
    fn navigation_rejects_unknown_tabs_and_keeps_monotonic_revisions() {
        assert!(serde_json::from_str::<View>("\"Codex\"").is_err());
        assert!(serde_json::from_str::<View>("\"Activity\"").is_err());
        let (tx, _) = sync_channel(1);
        let state = AppState::new(PathBuf::new(), Config::default(), tx);
        assert_eq!(select(&state, View::Agents).revision, 1);
        assert_eq!(
            select(&state, View::Deck),
            Navigation {
                view: View::Deck,
                revision: 2
            }
        );
    }
}
