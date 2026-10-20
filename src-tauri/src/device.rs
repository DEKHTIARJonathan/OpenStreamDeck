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
    AgentFrame(u64, Vec<Vec<u8>>, Vec<AgentSession>, String),
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
fn transmit_images(
    images: &[Vec<u8>],
    mut send: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    if images.len() != 6 {
        return Err("A display frame must contain all six LCD images".into());
    }
    for (index, bytes) in images.iter().enumerate() {
        for packet in protocol::image_packets((index + 1) as u8, bytes)? {
            send(&packet)?;
        }
    }
    send(&protocol::command(b"STP"))
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

#[derive(Default)]
struct AppliedDisplay {
    page: Option<Config>,
    agents: Option<Vec<AgentSession>>,
}
impl AppliedDisplay {
    fn reset(&mut self) {
        *self = Self::default();
    }
    fn capture(
        &self,
        control: &str,
        fallback: impl FnOnce() -> Config,
    ) -> crate::actions::CapturedTrigger {
        if let Some(agents) = &self.agents {
            // An applied overlay owns only the LCDs. Other controls follow the
            // current underlying page, including foreground changes and saves.
            crate::actions::capture_control(&fallback(), Some(agents), control)
        } else if let Some(config) = &self.page {
            crate::actions::capture_control(config, None, control)
        } else {
            crate::actions::CapturedTrigger::None
        }
    }
}

/// The same page snapshot supplies all six frames and subsequent input bindings.
/// Transport injection exercises production image chunking and commit ordering.
struct PageFrame {
    config: Config,
    generation: u64,
    images: Vec<Vec<u8>>,
}
impl PageFrame {
    fn prepare(
        mut config: Config,
        context: &crate::page_context::PageContext,
    ) -> Result<Self, String> {
        config.active_page = context.desired_page.clone();
        let page = config.page().ok_or("Selected page no longer exists")?;
        let images = (1..=6)
            .map(|key| {
                let action = page
                    .bindings
                    .get(&format!("lcd{key}"))
                    .cloned()
                    .unwrap_or_default();
                render(&action, config.image_size, config.rotation)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            config,
            generation: context.generation,
            images,
        })
    }

    fn transmit(&self, send: impl FnMut(&[u8]) -> Result<(), String>) -> Result<(), String> {
        transmit_images(&self.images, send)
    }
}

fn redraw(app: &tauri::AppHandle, d: &HidDevice) -> Result<Option<Config>, String> {
    let state = app.state::<AppState>();
    let (config, context) = {
        let config = state.config.lock().unwrap();
        let context = state.page_context.lock().unwrap();
        (config.clone(), context.clone())
    };
    if context.overlay {
        return Ok(None);
    }
    let frame = PageFrame::prepare(config, &context)?;
    // Rendering can be slower than focus changes. Skip obsolete work before USB.
    if state.page_context.lock().unwrap().generation != frame.generation {
        return Ok(None);
    }
    frame.transmit(|packet| write(app, d, packet))?;
    state
        .page_context
        .lock()
        .unwrap()
        .applied(frame.config.active_page.clone(), frame.generation);
    crate::page_context::publish(app);
    Ok(Some(frame.config))
}
pub fn worker(app: tauri::AppHandle, rx: Receiver<DeviceCommand>) {
    // Startup always connects; Disconnect only affects the current process.
    let mut wanted = true;
    let mut device: Option<HidDevice> = None;
    let mut retry = Instant::now();
    let mut heartbeat = Instant::now();
    let mut last_input = Instant::now();
    let mut applied = AppliedDisplay::default();
    let update = |message: String, connected: bool| {
        let state = app.state::<AppState>();
        let mut s = state.device.lock().unwrap();
        s.connected = connected;
        s.message = message;
        let _ = app.emit("device", s.clone());
        drop(s);
        if !connected {
            state.page_context.lock().unwrap().invalidate();
            crate::page_context::publish(&app);
        }
    };
    loop {
        if device.is_none() {
            applied.reset();
        }
        if app
            .state::<AppState>()
            .quitting
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return;
        }
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
                applied.reset();
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
                let w = {
                    let config = state.config.lock().unwrap();
                    state.page_context.lock().unwrap().effective_config(&config)
                };
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
                applied.reset();
                state.page_context.lock().unwrap().invalidate();
                crate::page_context::publish(&app);
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
                    device = None;
                    retry = Instant::now() + Duration::from_secs(2);
                    update("Hardware diagnostics failed; reconnecting".into(), false);
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
                    DeviceCommand::Redraw => {
                        app.state::<AppState>()
                            .page_context
                            .lock()
                            .unwrap()
                            .invalidate();
                        Ok(())
                    }
                    DeviceCommand::AgentFrame(epoch, images, sessions, fingerprint) => {
                        let state = app.state::<AppState>();
                        let sleeping = state.device.lock().unwrap().sleeping;
                        let context = state.page_context.lock().unwrap().clone();
                        if overlay_frame_allowed(&context, sleeping, epoch) {
                            let result = transmit_images(&images, |packet| write(&app, d, packet));
                            if result.is_ok() {
                                applied.agents = Some(sessions);
                                *state.agent_frame.lock().unwrap() = fingerprint;
                                state.page_context.lock().unwrap().applied_overlay();
                                crate::page_context::publish(&app);
                            }
                            result
                        } else {
                            state.agent_frame.lock().unwrap().clear();
                            Ok(())
                        }
                    }
                    DeviceCommand::Raw(b) => write(&app, d, &b),
                    DeviceCommand::Connect(_) | DeviceCommand::HardwareSmoke => Ok(()),
                };
                if let Err(e) = result {
                    let _ = app.emit("action_error", format!("Device command failed: {e}"));
                    device = None;
                    applied.reset();
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
                    let result = init.iter().try_for_each(|p| write(&app, &d, p));
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
        // Desired state is a mailbox, not a queue of focus events. Even a full
        // command queue cannot lose the final target. Sleeping defers all frames.
        if let Some(ref d) = device {
            let state = app.state::<AppState>();
            let sleeping = state.device.lock().unwrap().sleeping;
            if page_frame_allowed(&state.page_context.lock().unwrap(), sleeping) {
                match redraw(&app, d) {
                    Ok(Some(config)) => {
                        applied.page = Some(config);
                        applied.agents = None;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        let _ = app.emit("action_error", format!("Page display failed: {error}"));
                        applied.reset();
                        device = None;
                        retry = Instant::now() + Duration::from_secs(2);
                        update(error, false);
                    }
                }
            }
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
                                    state.agent_frame.lock().unwrap().clear();
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
                            let trigger = applied.capture(&event.control, || {
                                let config = state.config.lock().unwrap();
                                state.page_context.lock().unwrap().effective_config(&config)
                            });
                            let handle = app.clone();
                            tauri::async_runtime::spawn(async move {
                                crate::actions::dispatch_captured(handle, trigger).await;
                            });
                        }
                    }
                }
                Err(e) => {
                    device = None;
                    applied.reset();
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

fn overlay_frame_allowed(
    context: &crate::page_context::PageContext,
    sleeping: bool,
    epoch: u64,
) -> bool {
    !sleeping && context.overlay && context.overlay_epoch == epoch
}
fn page_frame_allowed(context: &crate::page_context::PageContext, sleeping: bool) -> bool {
    !sleeping && context.needs_frame()
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

#[cfg(test)]
mod context_frame_tests {
    use super::*;
    use crate::{actions::CapturedTrigger, page_context::PageContext};

    fn fixture() -> Config {
        let mut config = Config::default();
        let mut second = config.pages[0].clone();
        second.id = "application".into();
        second.name = "Application".into();
        for control in [
            "lcd1",
            "lcd2",
            "lcd3",
            "lcd4",
            "lcd5",
            "lcd6",
            "button1",
            "button2",
            "button3",
            "knob1",
            "knob2",
            "knob3",
            "knob1.cw",
            "knob1.ccw",
            "knob2.cw",
            "knob2.ccw",
            "knob3.cw",
            "knob3.ccw",
        ] {
            second.bindings.insert(
                control.into(),
                Action {
                    kind: "text".into(),
                    value: format!("application:{control}"),
                    title: format!("App {control}"),
                    ..Default::default()
                },
            );
        }
        config.pages.push(second);
        config
    }

    #[test]
    fn all_six_page_frames_use_canonical_preview_and_commit_only_after_complete_transfer() {
        assert!(transmit_images(&[], |_| panic!("incomplete frames must not write")).is_err());
        let config = fixture();
        let mut context = PageContext::new(&config);
        for page in ["home", "application"] {
            context.manual(&config, page).unwrap();
            let frame = PageFrame::prepare(config.clone(), &context).unwrap();
            let mut packets = Vec::new();
            frame
                .transmit(|p| {
                    packets.push(p.to_vec());
                    Ok(())
                })
                .unwrap();
            let mut cursor = 0;
            for key in 1..=6 {
                let header = &packets[cursor];
                assert_eq!(&header[6..9], b"BAT");
                assert_eq!(header[13], key);
                let length = u16::from_be_bytes([header[11], header[12]]) as usize;
                let count = length.div_ceil(1024);
                let mut reconstructed = packets[cursor + 1..cursor + 1 + count]
                    .iter()
                    .flat_map(|p| p[1..].iter().copied())
                    .collect::<Vec<_>>();
                reconstructed.truncate(length);
                let action = frame
                    .config
                    .page()
                    .unwrap()
                    .bindings
                    .get(&format!("lcd{key}"))
                    .cloned()
                    .unwrap_or_default();
                assert_eq!(
                    reconstructed,
                    crate::artwork::render(&action, config.image_size, config.rotation).unwrap()
                );
                cursor += 1 + count;
            }
            assert_eq!(packets[cursor], protocol::command(b"STP"));
            assert_eq!(cursor + 1, packets.len());
            for failure in [0, 1, cursor / 2, cursor] {
                let mut calls = 0;
                let result = frame.transmit(|_| {
                    let current = calls;
                    calls += 1;
                    if current == failure {
                        Err("short write / disconnected".into())
                    } else {
                        Ok(())
                    }
                });
                assert!(result.is_err());
                assert_eq!(calls, failure + 1);
            }
        }
    }

    #[test]
    fn every_trigger_captures_transmitted_page_before_async_dispatch() {
        let mut config = fixture();
        config.active_page = "application".into();
        let dir = tempfile::tempdir().unwrap();
        let (tx, _rx) = std::sync::mpsc::sync_channel(1);
        let state = AppState::new(dir.path().into(), config.clone(), tx);
        let codes = [
            1, 2, 3, 4, 5, 6, 0x25, 0x30, 0x31, 0x35, 0x33, 0x34, 0x50, 0x51, 0x90, 0x91, 0x60,
            0x61,
        ];
        for code in codes {
            let mut report = b"ACK\0\0OK\0\0".to_vec();
            report.extend([code, 1]);
            let event = protocol::decode(&report).unwrap();
            assert_eq!(input_effect(false, &event), InputEffect::Trigger);
            let captured = crate::actions::capture_control(&config, None, &event.control);
            state.config.lock().unwrap().active_page = "home".into();
            let CapturedTrigger::Action {
                action,
                control,
                page,
            } = captured
            else {
                panic!("missing trigger")
            };
            assert_eq!(control, event.control);
            assert_eq!(page, "application");
            if control == "button1" || control == "button2" {
                assert_eq!(action.kind, "page");
                assert_eq!(
                    action.value,
                    if control == "button1" {
                        "previous"
                    } else {
                        "next"
                    }
                );
            } else {
                assert_eq!(action.value, format!("application:{control}"));
            }
            if code < 0x50 {
                for release in [0, 2] {
                    report[10] = release;
                    assert_eq!(
                        input_effect(true, &protocol::decode(&report).unwrap()),
                        InputEffect::Release
                    );
                }
            }
            report[10] = 1;
            assert_eq!(
                input_effect(true, &protocol::decode(&report).unwrap()),
                InputEffect::Wake
            );
        }
    }

    #[test]
    fn overlay_frames_cannot_replace_manual_pages_or_wake_sleeping_device() {
        let config = fixture();
        let mut context = PageContext::new(&config);
        context.set_overlay(true);
        let epoch = context.overlay_epoch;
        assert!(overlay_frame_allowed(&context, false, epoch));
        assert!(!overlay_frame_allowed(&context, true, epoch));
        context.manual(&config, "application").unwrap();
        assert!(!overlay_frame_allowed(&context, false, epoch));
        context.set_overlay(true);
        assert!(!overlay_frame_allowed(&context, false, epoch));
        assert!(overlay_frame_allowed(
            &context,
            false,
            context.overlay_epoch
        ));
    }

    #[test]
    fn latest_page_mailbox_survives_full_command_queue_and_deferred_sleep() {
        let config = fixture();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let state = AppState::new(std::path::PathBuf::new(), config.clone(), tx);
        state.tx.try_send(DeviceCommand::Brightness(20)).unwrap();
        let mut context = state.page_context.lock().unwrap();
        for page in ["application", "home", "application"] {
            context.manual(&config, page).unwrap();
        }
        assert!(context.needs_frame());
        assert!(!page_frame_allowed(&context, true));
        assert!(page_frame_allowed(&context, false));
        let frame = PageFrame::prepare(config, &context).unwrap();
        assert_eq!(frame.config.active_page, "application");
        assert!(matches!(
            rx.try_recv().unwrap(),
            DeviceCommand::Brightness(20)
        ));
        assert!(rx.try_recv().is_err());
        // Deferral leaves the latest target intact until successful transmission.
        frame.transmit(|_| Ok(())).unwrap();
        context.applied(frame.config.active_page, frame.generation);
        assert!(!context.needs_frame());
        context.invalidate();
        assert!(context.needs_frame());
        assert!(context.applied_page.is_none());
        assert_eq!(context.desired_page, "application");
    }

    #[test]
    fn overlay_input_uses_the_transmitted_session_snapshot() {
        let config = fixture();
        let session = AgentSession {
            id: "fixture-child".into(),
            source: "fixture".into(),
            agent: "fixture".into(),
            title: "Public fixture".into(),
            status: "ready".into(),
            raw_status: "idle".into(),
            session_id: Some("technical-child".into()),
            target: "technical-child".into(),
            updated_at: 0,
            parent_session_id: Some("parent".into()),
            is_subagent: true,
            can_focus: true,
            can_prompt: false,
        };
        let mut sessions = vec![session];
        let captured = crate::actions::capture_control(&config, Some(&sessions), "lcd1");
        sessions[0].target = "different".into();
        let CapturedTrigger::Agent(selected) = captured else {
            panic!("expected captured agent")
        };
        assert_eq!(selected.target, "technical-child");
        assert!(matches!(
            crate::actions::capture_control(&config, Some(&sessions), "lcd2"),
            CapturedTrigger::None
        ));
        assert!(matches!(
            crate::actions::capture_control(&config, None, "lcd1"),
            CapturedTrigger::Action { .. }
        ));
        assert!(matches!(
            crate::actions::capture_control(&config, Some(&sessions), "button1"),
            CapturedTrigger::Action { .. }
        ));
        let mut applied = AppliedDisplay {
            page: Some(config),
            agents: Some(sessions),
        };
        // Requesting an overlay alone must preserve the transmitted page's
        // bindings. Only a successfully transmitted overlay changes ownership.
        let transmitted_agents = applied.agents.take();
        let CapturedTrigger::Action { page, .. } = applied.capture("lcd1", || {
            panic!("a pending overlay must retain the displayed page")
        }) else {
            panic!("expected the displayed page binding")
        };
        assert_eq!(page, "home");
        applied.agents = transmitted_agents;

        // While the LCD overlay remains applied, foreground changes and saved
        // edits must reach every ordinary control without retransmitting LCDs.
        let mut current = fixture();
        current.active_page = "application".into();
        current.pages[1].bindings.get_mut("knob1.cw").unwrap().value = "edited".into();
        for control in [
            "button1",
            "button2",
            "button3",
            "knob1",
            "knob2",
            "knob3",
            "knob1.cw",
            "knob1.ccw",
            "knob2.cw",
            "knob2.ccw",
            "knob3.cw",
            "knob3.ccw",
        ] {
            let CapturedTrigger::Action { page, action, .. } =
                applied.capture(control, || current.clone())
            else {
                panic!("missing current underlying binding for {control}")
            };
            assert_eq!(page, "application");
            match control {
                "button1" | "button2" => {
                    assert_eq!(action.kind, "page");
                    assert_eq!(
                        action.value,
                        if control == "button1" {
                            "previous"
                        } else {
                            "next"
                        }
                    );
                }
                "knob1.cw" => assert_eq!(action.value, "edited"),
                _ => assert_eq!(action.value, format!("application:{control}")),
            }
        }
        let CapturedTrigger::Agent(session) = applied.capture("lcd1", || current.clone()) else {
            panic!("LCD must still use the transmitted agent session")
        };
        assert_eq!(session.id, "fixture-child");
        assert!(matches!(
            applied.capture("lcd2", || current),
            CapturedTrigger::None
        ));

        // Connect/reconnect and diagnostics discard both kinds of confirmed input.
        applied.reset();
        for control in ["lcd1", "button1", "knob1.cw"] {
            assert!(matches!(
                applied.capture(control, || panic!(
                    "no display may request fallback bindings"
                )),
                CapturedTrigger::None
            ));
        }
    }

    #[test]
    fn diagnostics_restores_runtime_target_instead_of_saved_manual_page() {
        let saved = fixture();
        let mut context = PageContext::new(&saved);
        context.manual(&saved, "application").unwrap();
        let effective = context.effective_config(&saved);
        assert_eq!(saved.active_page, "home");
        let plan = crate::diagnostics::plan(&effective, false).unwrap();
        let mut diagnostic_packets = vec![];
        let report = crate::diagnostics::execute(&plan, |packet| {
            diagnostic_packets.push(packet.to_vec());
            Ok(packet.len())
        });
        assert!(report.passed);
        let mut page_packets = vec![];
        PageFrame::prepare(saved, &context)
            .unwrap()
            .transmit(|packet| {
                page_packets.push(packet.to_vec());
                Ok(())
            })
            .unwrap();
        assert_eq!(
            &diagnostic_packets[6..diagnostic_packets.len() - 1],
            page_packets.as_slice()
        );
    }
}
