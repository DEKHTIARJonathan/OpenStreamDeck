//! Native foreground observation. The owned guard stops callbacks and joins the
//! observer thread; no webview, title collection, or shell polling is involved.
use crate::model::ApplicationIdentity;
use serde::Serialize;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct ForegroundApplication {
    pub name: String,
    pub identity: ApplicationIdentity,
}
pub type Observation = Result<Option<ForegroundApplication>, String>;
pub type Callback = Arc<dyn Fn(Observation) + Send + Sync>;

/// A failed/missing query retries even if the OS sends no new activation.
/// Successful detection is event driven, avoiding periodic foreground polling.
#[derive(Default)]
struct QuerySchedule {
    retry_at_ms: Option<u64>,
}
impl QuerySchedule {
    fn due(&self, changed: bool, now_ms: u64) -> bool {
        changed || self.retry_at_ms.is_some_and(|deadline| now_ms >= deadline)
    }
    fn observed(&mut self, observation: &Observation, now_ms: u64) {
        self.retry_at_ms = if matches!(observation, Ok(Some(_))) {
            None
        } else {
            Some(now_ms.saturating_add(2_000))
        };
    }
}

pub struct ForegroundObserver {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Drop for ForegroundObserver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
pub fn platform() -> &'static str {
    std::env::consts::OS
}
pub fn start(callback: Callback) -> Result<ForegroundObserver, String> {
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let thread = std::thread::Builder::new()
        .name("foreground-application".into())
        .spawn(move || {
            #[cfg(target_os = "macos")]
            macos::observe(worker_stop, callback);
            #[cfg(windows)]
            windows::observe(worker_stop, callback);
            #[cfg(target_os = "linux")]
            x11::observe(worker_stop, callback);
        })
        .map_err(|e| format!("Could not start application detection: {e}"))?;
    Ok(ForegroundObserver {
        stop,
        thread: Some(thread),
    })
}

pub fn application_identity(path: &str) -> Result<ApplicationIdentity, String> {
    if !std::path::Path::new(path).exists() {
        return Err("Application no longer exists; select an installed application".into());
    }
    #[cfg(target_os = "macos")]
    let value = macos::identity(path)?;
    #[cfg(windows)]
    let value = windows::identity(path)?;
    #[cfg(target_os = "linux")]
    let value = {
        if std::path::Path::new(path)
            .extension()
            .is_none_or(|extension| extension != "desktop")
        {
            return Err("Choose an installed application desktop entry (.desktop)".into());
        }
        desktop_identity(&std::fs::read_to_string(path).map_err(|e| e.to_string())?)?
    };
    Ok(ApplicationIdentity {
        platform: platform().into(),
        value,
    })
}

/// The picker and X11 observer use the resource class (second WM_CLASS value).
/// Never infer it from Name, Exec, window titles, or generic wrapper executables.
#[cfg(any(target_os = "linux", test))]
fn desktop_identity(text: &str) -> Result<String, String> {
    let mut in_entry = false;
    let mut class = None;
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
        } else if in_entry {
            if let Some(value) = line.strip_prefix("StartupWMClass=") {
                if class.is_some() {
                    return Err("Desktop entry has ambiguous StartupWMClass metadata".into());
                }
                class = Some(value.trim());
            }
        }
    }
    let class = class.filter(|value| {
        !value.is_empty() && value.len() <= 512 && !value.chars().any(char::is_control)
    }).ok_or("This desktop entry has no usable StartupWMClass. Automatic pages on X11 require that application metadata; generic launchers cannot be matched safely.")?;
    Ok(format!("wmclass:{class}"))
}

#[cfg(any(target_os = "linux", test))]
fn x11_identity(bytes: &[u8]) -> Result<String, String> {
    let fields: Vec<_> = bytes.split(|byte| *byte == 0).collect();
    if fields.len() != 3 || !fields[2].is_empty() || fields[1].is_empty() {
        return Err("Foreground X11 application has no usable WM_CLASS resource class".into());
    }
    let class = std::str::from_utf8(fields[1])
        .map_err(|_| "Foreground X11 application has invalid WM_CLASS metadata")?;
    if class.len() > 512 || class.chars().any(char::is_control) {
        return Err("Foreground X11 application has invalid WM_CLASS metadata".into());
    }
    Ok(format!("wmclass:{class}"))
}

#[cfg(any(windows, test))]
fn executable_identity(path: &str) -> Result<String, String> {
    let normalized = path.replace('/', "\\").to_lowercase();
    let normalized = normalized.strip_prefix("\\\\?\\").unwrap_or(&normalized);
    let filename = normalized.rsplit('\\').next().unwrap_or_default();
    if matches!(
        filename,
        "applicationframehost.exe" | "wwahost.exe" | "rundll32.exe" | "explorer.exe"
    ) {
        return Err("This application uses a shared Windows host; an exact application identity is unavailable".into());
    }
    if normalized.len() > 4096
        || !filename.ends_with(".exe")
        || normalized
            .as_bytes()
            .get(1..3)
            .is_none_or(|bytes| bytes != b":\\")
        || normalized.chars().any(char::is_control)
    {
        return Err("Application identity must resolve to an absolute executable path".into());
    }
    Ok(normalized.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn linux_picker_and_observer_share_exact_resource_class() {
        let desktop = "[Desktop Entry]\nName=Chrome\nExec=/bin/wrapper %U\nStartupWMClass=Google-chrome\n[Desktop Action New]\nStartupWMClass=Wrong\n";
        assert_eq!(
            desktop_identity(desktop).unwrap(),
            x11_identity(b"chrome\0Google-chrome\0").unwrap()
        );
        assert_ne!(
            x11_identity(b"chrome\0Google-chrome\0").unwrap(),
            x11_identity(b"chrome\0google-chrome\0").unwrap()
        );
        for invalid in [
            "[Desktop Entry]\nExec=chrome",
            "[Desktop Entry]\nStartupWMClass=",
            "[Other]\nStartupWMClass=Chrome",
            "[Desktop Entry]\nStartupWMClass=A\nStartupWMClass=B",
        ] {
            assert!(desktop_identity(invalid).is_err());
        }
        for invalid in [
            b"no separator".as_slice(),
            b"instance\0\0",
            b"instance\0class",
            b"i\0c\0extra",
            b"i\0\xff\0",
        ] {
            assert!(x11_identity(invalid).is_err());
        }
    }
    #[test]
    fn windows_identity_is_exact_normalized_and_rejects_shared_hosts() {
        assert_eq!(
            executable_identity("C:/Apps/Code.EXE").unwrap(),
            executable_identity("\\\\?\\C:\\Apps\\code.exe").unwrap()
        );
        for path in [
            "code.exe",
            "C:\\Apps\\shortcut.lnk",
            "C:\\Windows\\ApplicationFrameHost.exe",
            "C:\\Windows\\explorer.exe",
            "C:\\Windows\\rundll32.exe",
            "C:\\app\n.exe",
        ] {
            assert!(executable_identity(path).is_err(), "{path}");
        }
    }
    #[test]
    fn missing_picker_target_is_explicit() {
        assert!(
            application_identity("/definitely-not-an-installed-application")
                .unwrap_err()
                .contains("no longer exists")
        );
    }
    #[test]
    fn observation_retries_recover_without_polling_healthy_applications() {
        let mut schedule = QuerySchedule::default();
        assert!(schedule.due(true, 0)); // subscription is installed before initial invalidation
        schedule.observed(&Err("permission unavailable".into()), 100);
        assert!(!schedule.due(false, 2_099));
        assert!(schedule.due(false, 2_100));
        assert!(schedule.due(true, 101)); // a new activation never waits for retry
        schedule.observed(&Ok(None), 2_100);
        assert!(!schedule.due(false, 4_099));
        assert!(schedule.due(false, 4_100));
        schedule.observed(
            &Ok(Some(ForegroundApplication {
                name: "Example".into(),
                identity: ApplicationIdentity {
                    platform: "macos".into(),
                    value: "org.example.app".into(),
                },
            })),
            4_100,
        );
        assert!(!schedule.due(false, u64::MAX));
        assert!(schedule.due(true, 4_101));
    }
    #[test]
    fn dropping_observer_stops_and_joins_owned_worker() {
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let finished = Arc::new(AtomicBool::new(false));
        let worker_finished = finished.clone();
        let thread = std::thread::spawn(move || {
            while !worker_stop.load(Ordering::Acquire) {
                std::thread::yield_now();
            }
            worker_finished.store(true, Ordering::Release);
        });
        drop(ForegroundObserver {
            stop,
            thread: Some(thread),
        });
        assert!(finished.load(Ordering::Acquire));
    }
}
