use super::*;
use std::cell::Cell;
use std::time::{Duration, Instant};
use windows_sys::Win32::{
    Foundation::{CloseHandle, HWND},
    System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    },
    UI::{
        Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK},
        WindowsAndMessaging::{
            DispatchMessageW, GetForegroundWindow, GetWindowThreadProcessId, PeekMessageW,
            TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG, PM_REMOVE, WINEVENT_OUTOFCONTEXT,
        },
    },
};
thread_local! { static DIRTY: Cell<bool> = const { Cell::new(true) }; }
unsafe extern "system" fn changed(
    _: HWINEVENTHOOK,
    _: u32,
    _: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    // Out-of-context hooks deliver on the installing thread. Query the current
    // handle after draining events, never an old event's HWND or window title.
    DIRTY.set(true);
}
fn current() -> Observation {
    let window = unsafe { GetForegroundWindow() };
    if window.is_null() {
        return Ok(None);
    }
    let mut pid = 0;
    unsafe {
        GetWindowThreadProcessId(window, &mut pid);
    }
    if pid == 0 {
        return Ok(None);
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
        return Err(
            "Foreground application identity is inaccessible (Windows process permissions)".into(),
        );
    }
    let mut buffer = vec![0u16; 32768];
    let mut len = buffer.len() as u32;
    let success = unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, buffer.as_mut_ptr(), &mut len)
    };
    unsafe {
        CloseHandle(process);
    }
    if success == 0 {
        return Err("Windows could not resolve the foreground application's executable".into());
    }
    // A focus change during lookup invalidates this result. The queued event
    // will request a new query, avoiding publication of a stale process.
    if unsafe { GetForegroundWindow() } != window {
        DIRTY.set(true);
        return Ok(None);
    }
    let path = String::from_utf16(&buffer[..len as usize])
        .map_err(|_| "Application executable path is not valid Unicode")?;
    let value = executable_identity(&path)?;
    let name = path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(&path)
        .trim_end_matches(".exe")
        .into();
    Ok(Some(ForegroundApplication {
        name,
        identity: ApplicationIdentity {
            platform: "windows".into(),
            value,
        },
    }))
}
pub(super) fn observe(stop: Arc<AtomicBool>, callback: Callback) {
    while !stop.load(Ordering::Acquire) {
        let hook = unsafe {
            SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                std::ptr::null_mut(),
                Some(changed),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            )
        };
        if hook.is_null() {
            callback(Err("Windows foreground event registration failed".into()));
            for _ in 0..100 {
                if stop.load(Ordering::Acquire) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            continue;
        }
        DIRTY.set(true);
        let origin = Instant::now();
        let mut schedule = QuerySchedule::default();
        while !stop.load(Ordering::Acquire) {
            let mut message: MSG = unsafe { std::mem::zeroed() };
            while unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } != 0
            {
                unsafe {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
            if schedule.due(DIRTY.replace(false), origin.elapsed().as_millis() as u64) {
                let result = current();
                schedule.observed(&result, origin.elapsed().as_millis() as u64);
                callback(result);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        unsafe {
            UnhookWinEvent(hook);
        }
    }
}

pub(super) fn identity(path: &str) -> Result<String, String> {
    use windows::core::{Interface, PCWSTR};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
        COINIT_APARTMENTTHREADED, STGM_READ,
    };
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    let target = if std::path::Path::new(path)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("lnk"))
    {
        // Called on a dedicated blocking worker; COM interfaces never escape its apartment.
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }
            .map_err(|e| format!("Could not initialize shortcut resolution: {e}"))?;
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _apartment = Apartment;
        let link: IShellLinkW = unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }
            .map_err(|e| e.to_string())?;
        let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
        let path: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
        unsafe { file.Load(PCWSTR(path.as_ptr()), STGM_READ) }
            .map_err(|e| format!("Could not read application shortcut: {e}"))?;
        let mut target = vec![0u16; 32768];
        unsafe { link.GetPath(&mut target, std::ptr::null_mut(), 0) }
            .map_err(|e| format!("Could not resolve application shortcut: {e}"))?;
        let len = target
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(target.len());
        String::from_utf16(&target[..len])
            .map_err(|_| "Application shortcut has an invalid target")?
    } else {
        path.into()
    };
    if !std::path::Path::new(&target).is_file() {
        return Err(
            "Application shortcut target no longer exists or is not a direct executable".into(),
        );
    }
    let canonical = std::fs::canonicalize(&target)
        .map_err(|e| format!("Could not resolve application executable: {e}"))?;
    executable_identity(&canonical.to_string_lossy())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shortcut_identity_resolves_target_without_executing_it() {
        use windows::core::{Interface, PCWSTR};
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CoUninitialize, IPersistFile, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        };
        use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("Synthetic.exe");
        let shortcut = directory.path().join("Different display name.lnk");
        std::fs::write(&executable, b"not an executable; never launched").unwrap();
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok() }.unwrap();
        {
            let link: IShellLinkW =
                unsafe { CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER) }.unwrap();
            let target: Vec<_> = executable
                .to_str()
                .unwrap()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            unsafe { link.SetPath(PCWSTR(target.as_ptr())) }.unwrap();
            let file: IPersistFile = link.cast().unwrap();
            let path: Vec<_> = shortcut
                .to_str()
                .unwrap()
                .encode_utf16()
                .chain(Some(0))
                .collect();
            unsafe { file.Save(PCWSTR(path.as_ptr()), true) }.unwrap();
        }
        unsafe {
            CoUninitialize();
        }
        assert_eq!(
            identity(shortcut.to_str().unwrap()).unwrap(),
            identity(executable.to_str().unwrap()).unwrap()
        );
        std::fs::remove_file(&executable).unwrap();
        assert!(identity(shortcut.to_str().unwrap())
            .unwrap_err()
            .contains("no longer exists"));
    }
}
