use crate::{model::*, protocol, AppState};
use hidapi::{HidApi, HidDevice};
use serde::Serialize;
use std::{
    io::Write,
    sync::mpsc::{Receiver, TryRecvError},
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager};

pub enum DeviceCommand {
    Connect(bool),
    Brightness(u8),
    Sleep(bool),
    ToggleSleep,
    Redraw,
    Image(u8, Vec<u8>),
    Raw(Vec<u8>),
    HardwareSmoke,
}
impl DeviceCommand {
    pub fn requires_connection(&self) -> bool {
        !matches!(self, Self::Connect(_) | Self::HardwareSmoke)
    }

    fn sleep_target(&self, current: bool) -> Option<bool> {
        match self {
            Self::Sleep(sleep) => Some(*sleep),
            Self::ToggleSleep => Some(!current),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum InputEffect {
    Release,
    Wake,
    Trigger,
}

fn input_effect(sleeping: bool, event: &DeviceEvent) -> InputEffect {
    if event.event == "up" {
        InputEffect::Release
    } else if sleeping {
        // Consume the wake press/rotation so it cannot also run its binding.
        InputEffect::Wake
    } else {
        InputEffect::Trigger
    }
}
#[derive(Serialize)]
pub struct Candidate {
    pub serial: String,
    pub product: String,
    pub path: String,
    pub usage_page: u16,
    pub usage: u16,
}
pub fn enumerate() -> Result<Vec<Candidate>, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    Ok(api
        .device_list()
        .filter(|d| d.vendor_id() == 0x5548 && d.product_id() == 0x1001)
        .map(|d| Candidate {
            serial: d.serial_number().unwrap_or_default().into(),
            product: d.product_string().unwrap_or_default().into(),
            path: d.path().to_string_lossy().into(),
            usage_page: d.usage_page(),
            usage: d.usage(),
        })
        .collect())
}
fn open_device() -> Result<(HidDevice, String), String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let info = api
        .device_list()
        .find(|d| {
            d.vendor_id() == 0x5548
                && d.product_id() == 0x1001
                && d.usage_page() == 0xffa0
                && d.usage() == 1
        })
        .ok_or("N3 vendor HID interface not found")?;
    let device = info.open_device(&api).map_err(|e| e.to_string())?;
    Ok((device, info.serial_number().unwrap_or_default().into()))
}
fn trace(app: &tauri::AppHandle, direction: &str, b: &[u8], result: Option<usize>) {
    if std::env::var("OPENSTREAMDECK_USB_TRACE").as_deref() != Ok("1") {
        return;
    }
    let state = app.state::<AppState>();
    let p = state.dir.join("usb-trace.jsonl");
    if p.metadata().map(|m| m.len() > 10_000_000).unwrap_or(false) {
        let _ = std::fs::rename(&p, state.dir.join("usb-trace.previous.jsonl"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(p)
    {
        let _ = writeln!(
            f,
            "{}",
            serde_json::json!({"at":now(),"direction":direction,"length":b.len(),"result":result,"hex":b.iter().map(|v|format!("{v:02x}")).collect::<String>(),"boundary":"hidapi"})
        );
    }
}
fn write(app: &tauri::AppHandle, d: &HidDevice, b: &[u8]) -> Result<(), String> {
    let n = d.write(b).map_err(|e| e.to_string())?;
    trace(app, "out", b, Some(n));
    if n != b.len() {
        return Err(format!("Short HID write: {n}/{}", b.len()));
    }
    app.state::<AppState>().device.lock().unwrap().packets_out += 1;
    Ok(())
}
fn image(app: &tauri::AppHandle, d: &HidDevice, key: u8, bytes: &[u8]) -> Result<(), String> {
    for p in protocol::image_packets(key, bytes)? {
        write(app, d, &p)?
    }
    Ok(())
}
pub fn encode_image(bytes: &[u8], size: u32, rotation: u16) -> Result<Vec<u8>, String> {
    if bytes.len() > 2_000_000
        || !(32..=128).contains(&size)
        || ![0, 90, 180, 270].contains(&rotation)
    {
        return Err("Invalid artwork size, rotation or encoded image budget".into());
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().map_err(|e| e.to_string())?.resize_exact(
        size,
        size,
        image::imageops::FilterType::Lanczos3,
    );
    let img = match rotation {
        90 => img.rotate90(),
        180 => img.rotate180(),
        270 => img.rotate270(),
        _ => img,
    };
    let mut out = vec![];
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90)
        .encode_image(&img.to_rgb8())
        .map_err(|e| e.to_string())?;
    Ok(out)
}
pub fn render(a: &Action, size: u32, rotation: u16) -> Result<Vec<u8>, String> {
    crate::artwork::render(a, size, rotation)
}

fn redraw(app: &tauri::AppHandle, d: &HidDevice) -> Result<(), String> {
    let w = app.state::<AppState>().config.lock().unwrap().clone();
    if let Some(p) = w.page() {
        for key in 1..=6 {
            let a = p
                .bindings
                .get(&format!("lcd{key}"))
                .cloned()
                .unwrap_or_default();
            image(app, d, key, &render(&a, w.image_size, w.rotation)?)?
        }
        write(app, d, &protocol::command(b"STP"))?;
    }
    Ok(())
}
pub fn worker(app: tauri::AppHandle, rx: Receiver<DeviceCommand>) {
    // Startup always connects; Disconnect only affects the current process.
    let mut wanted = true;
    let mut device: Option<HidDevice> = None;
    let mut retry = Instant::now();
    let mut heartbeat = Instant::now();
    let mut last_input = Instant::now();
    let update = |message: String, connected: bool| {
        let state = app.state::<AppState>();
        let mut s = state.device.lock().unwrap();
        s.connected = connected;
        s.message = message;
        let _ = app.emit("device", s.clone());
    };
    loop {
        let mut commands = vec![];
        for _ in 0..16 {
            match rx.try_recv() {
                Ok(c) => commands.push(c),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return,
            }
        }
        for c in commands {
            if let DeviceCommand::Connect(connect) = c {
                wanted = connect;
                device = None;
                retry = Instant::now();
                update(
                    if connect {
                        "Connecting…"
                    } else {
                        "Disconnected"
                    }
                    .into(),
                    false,
                );
                continue;
            }
            if matches!(c, DeviceCommand::HardwareSmoke) {
                let state = app.state::<AppState>();
                let w = state.config.lock().unwrap().clone();
                let sleeping = state.device.lock().unwrap().sleeping;
                let report = match (&device, crate::diagnostics::plan(&w, sleeping)) {
                    (Some(d), Ok(plan)) => crate::diagnostics::execute(&plan, |bytes| {
                        write(&app, d, bytes).map(|_| bytes.len())
                    }),
                    (None, _) => {
                        crate::diagnostics::SmokeReport::unavailable("N3 is not connected".into())
                    }
                    (_, Err(e)) => crate::diagnostics::SmokeReport::unavailable(e),
                };
                state.agent_frame.lock().unwrap().clear();
                let result = serde_json::to_vec_pretty(&report)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        std::fs::write(state.dir.join("hardware-smoke.json"), bytes)
                            .map_err(|e| e.to_string())
                    });
                if let Err(error) = result {
                    let _ = app.emit("action_error", error);
                }
                if !report.passed {
                    let _ = app.emit(
                        "action_error",
                        "Hardware smoke failed; see hardware-smoke.json in app data",
                    );
                }
                last_input = Instant::now();
                continue;
            }
            if device.is_none() {
                let _ = app.emit(
                    "action_error",
                    "Device command could not run: N3 disconnected",
                );
                continue;
            }
            if let Some(ref d) = device {
                let result = match c {
                    DeviceCommand::Brightness(n) => write(&app, d, &protocol::brightness(n)),
                    DeviceCommand::Sleep(_) | DeviceCommand::ToggleSleep => {
                        // Resolve toggles in the serialized worker, not against a
                        // potentially stale snapshot in the action dispatcher.
                        let current = app.state::<AppState>().device.lock().unwrap().sleeping;
                        let sleep = c.sleep_target(current).unwrap();
                        let result = write(
                            &app,
                            d,
                            &protocol::command(if sleep { b"HAN" } else { b"DIS" }),
                        );
                        if result.is_ok() {
                            app.state::<AppState>().device.lock().unwrap().sleeping = sleep;
                            last_input = Instant::now();
                            update(if sleep { "Sleeping" } else { "Connected" }.into(), true);
                        }
                        result
                    }
                    DeviceCommand::Redraw => redraw(&app, d),
                    DeviceCommand::Image(key, bytes) => image(&app, d, key, &bytes)
                        .and_then(|_| write(&app, d, &protocol::command(b"STP"))),
                    DeviceCommand::Raw(b) => write(&app, d, &b),
                    DeviceCommand::Connect(_) | DeviceCommand::HardwareSmoke => Ok(()),
                };
                if let Err(e) = result {
                    let _ = app.emit("action_error", format!("Device command failed: {e}"));
                    device = None;
                    retry = Instant::now() + Duration::from_secs(2);
                    update(e, false);
                }
            }
        }
        if wanted && device.is_none() && Instant::now() >= retry {
            match open_device() {
                Ok((d, serial)) => {
                    let n = app.state::<AppState>().config.lock().unwrap().brightness;
                    let init = [
                        protocol::command(b"DIS"),
                        protocol::brightness(n),
                        protocol::command(b"STP"),
                        protocol::command(&[b'C', b'L', b'E', 0, 0, 0, 255]),
                    ];
                    let result = init
                        .iter()
                        .try_for_each(|p| write(&app, &d, p))
                        .and_then(|_| redraw(&app, &d));
                    match result {
                        Ok(()) => {
                            let state = app.state::<AppState>();
                            {
                                let mut s = state.device.lock().unwrap();
                                s.serial = serial;
                                s.product = "N3".into();
                                s.sleeping = false;
                            }
                            state.agent_frame.lock().unwrap().clear();
                            device = Some(d);
                            heartbeat = Instant::now();
                            last_input = Instant::now();
                            update("Connected · HID 5548:1001".into(), true)
                        }
                        Err(e) => update(e, false),
                    }
                }
                Err(e) => update(e, false),
            }
            retry = Instant::now() + Duration::from_secs(3);
        }
        if let Some(ref d) = device {
            let mut buf = [0; 512];
            match d.read_timeout(&mut buf, 10) {
                Ok(n) if n > 0 => {
                    trace(&app, "in", &buf[..n], Some(n));
                    app.state::<AppState>().device.lock().unwrap().packets_in += 1;
                    if let Some(event) = protocol::decode(&buf[..n]) {
                        last_input = Instant::now();
                        let state = app.state::<AppState>();
                        let sleeping = state.device.lock().unwrap().sleeping;
                        let _ = app.emit("input", &event);
                        let effect = input_effect(sleeping, &event);
                        if effect == InputEffect::Wake {
                            match write(&app, d, &protocol::command(b"DIS")) {
                                Ok(()) => {
                                    state.device.lock().unwrap().sleeping = false;
                                    update("Connected".into(), true);
                                }
                                Err(error) => {
                                    let _ = app.emit(
                                        "action_error",
                                        format!("Could not wake N3: {error}"),
                                    );
                                    device = None;
                                    retry = Instant::now() + Duration::from_secs(2);
                                    update(error, false);
                                }
                            }
                        } else if effect == InputEffect::Trigger {
                            let handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                crate::actions::trigger_control(handle, event.control).await;
                            });
                        }
                    }
                }
                Err(e) => {
                    device = None;
                    retry = Instant::now() + Duration::from_secs(2);
                    update(e.to_string(), false)
                }
                _ => {}
            }
        } else {
            std::thread::sleep(Duration::from_millis(50))
        }
        if let Some(ref d) = device {
            if heartbeat.elapsed() > Duration::from_secs(5) {
                if let Err(e) = write(&app, d, &protocol::command(b"CONNECT")) {
                    update(e, false);
                    device = None;
                }
                heartbeat = Instant::now();
            }
        }
        let idle = app.state::<AppState>().config.lock().unwrap().sleep_minutes;
        if idle > 0 && last_input.elapsed() > Duration::from_secs(u64::from(idle) * 60) {
            if let Some(ref d) = device {
                let state = app.state::<AppState>();
                let sleeping = state.device.lock().unwrap().sleeping;
                if !sleeping {
                    match write(&app, d, &protocol::command(b"HAN")) {
                        Ok(()) => {
                            state.device.lock().unwrap().sleeping = true;
                            update("Sleeping".into(), true);
                        }
                        Err(error) => {
                            device = None;
                            retry = Instant::now() + Duration::from_secs(2);
                            update(error, false);
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod sleep_input_tests {
    use super::*;

    fn event(code: u8, state: u8) -> DeviceEvent {
        let mut report = b"ACK\0\0OK\0\0".to_vec();
        report.extend([code, state]);
        protocol::decode(&report).unwrap()
    }

    #[test]
    fn releases_never_wake_or_dispatch_and_wake_inputs_are_consumed() {
        for code in [1, 2, 3, 4, 5, 6, 0x25, 0x30, 0x31, 0x35, 0x33, 0x34] {
            for release in [0, 2] {
                for sleeping in [false, true] {
                    assert_eq!(
                        input_effect(sleeping, &event(code, release)),
                        InputEffect::Release
                    );
                }
            }
            assert_eq!(input_effect(false, &event(code, 1)), InputEffect::Trigger);
            assert_eq!(input_effect(true, &event(code, 1)), InputEffect::Wake);
        }
        for code in [0x50, 0x51, 0x90, 0x91, 0x60, 0x61] {
            assert_eq!(input_effect(false, &event(code, 0)), InputEffect::Trigger);
            assert_eq!(input_effect(true, &event(code, 0)), InputEffect::Wake);
        }
    }

    #[test]
    fn sleep_stays_latched_on_release_and_queued_toggles_use_current_state() {
        let press = event(0x31, 1);
        assert_eq!(input_effect(false, &press), InputEffect::Trigger);
        let sleeping = DeviceCommand::ToggleSleep.sleep_target(false).unwrap();
        assert!(sleeping);
        assert_eq!(
            input_effect(sleeping, &event(0x31, 0)),
            InputEffect::Release
        );
        assert_eq!(input_effect(sleeping, &press), InputEffect::Wake);
        assert_eq!(input_effect(false, &event(0x31, 2)), InputEffect::Release);
        assert_eq!(
            DeviceCommand::ToggleSleep.sleep_target(sleeping),
            Some(false)
        );
        for current in [true, false] {
            assert_eq!(
                DeviceCommand::Sleep(false).sleep_target(current),
                Some(false)
            );
            assert_eq!(DeviceCommand::Sleep(true).sleep_target(current), Some(true));
        }
        assert!(DeviceCommand::ToggleSleep.requires_connection());
    }
}

#[cfg(test)]
mod image_limit_tests {
    use super::*;
    use image::ImageEncoder;
    #[test]
    fn artwork_decode_limits_and_geometry_fail_without_hardware() {
        for (size, rotation) in [(0, 0), (129, 0), (64, 45)] {
            assert!(encode_image(&[], size, rotation).is_err());
        }
        assert!(encode_image(&vec![0; 2_000_001], 64, 0).is_err());
        assert!(encode_image(b"not an image", 64, 0).is_err());
        let mut png = vec![];
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&vec![0; 4097 * 3], 4097, 1, image::ExtendedColorType::Rgb8)
            .unwrap();
        let error = encode_image(&png, 64, 0).unwrap_err();
        assert!(error.to_lowercase().contains("limit"), "{error}");
    }
}
