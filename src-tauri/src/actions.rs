use crate::{device::DeviceCommand, model::*, AppState};
use enigo::{Direction, Enigo, Key, Keyboard, Mouse, Settings};
use std::{
    future::Future,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tauri::{Emitter, Manager};
use tokio::io::AsyncReadExt;

// The two navigation buttons are reserved hardware controls. Configured
// bindings remain preserved in configuration, but cannot change these roles.
fn fixed_navigation_action(control: &str) -> Option<Action> {
    match control {
        "button1" | "button2" => Some(Action {
            kind: "page".into(),
            title: if control == "button1" {
                "Previous page"
            } else {
                "Next page"
            }
            .into(),
            value: if control == "button1" {
                "previous"
            } else {
                "next"
            }
            .into(),
            ..Default::default()
        }),
        _ => None,
    }
}

pub(crate) fn control_action(config: &Config, control: &str) -> Option<Action> {
    fixed_navigation_action(control).or_else(|| config.page()?.bindings.get(control).cloned())
}

pub async fn trigger_control(app: tauri::AppHandle, control: String) {
    let state = app.state::<AppState>();
    if state.agent_display.load(Ordering::Relaxed) && control.starts_with("lcd") {
        if let Ok(n) = control[3..].parse::<usize>() {
            let s = {
                let agents = state.agents.lock().unwrap();
                let selected = *state.selected_agent.lock().unwrap();
                agents.get((selected / 6) * 6 + n - 1).cloned()
            };
            if let Some(s) = s {
                if let Err(error) = crate::agents::focus(&s).await {
                    let _ = app.emit("action_error", format!("Focus {}: {error}", s.title));
                }
            }
            return;
        }
    }
    let action = control_action(&state.config.lock().unwrap(), &control);
    if let Some(a) = action {
        start(app, a, control).await;
    }
}
pub async fn start(app: tauri::AppHandle, action: Action, binding: String) -> String {
    let action = fixed_navigation_action(&binding).unwrap_or(action);
    let id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let state = app.state::<AppState>();
        let mut active = state.active.lock().unwrap();
        if state.update_installing.load(Ordering::SeqCst) {
            let _ = app.emit(
                "action_error",
                "An update is in progress. OpenStreamDeck will restart automatically.",
            );
            return String::new();
        }
        if let Some(existing) = active
            .get(&binding)
            .filter(|_| !["brightness", "media", "agent_select"].contains(&action.kind.as_str()))
        {
            return existing.clone();
        }
        active.insert(binding.clone(), id.clone());
        state
            .cancellations
            .lock()
            .unwrap()
            .insert(id.clone(), cancel.clone());
    }
    let run_id = id.clone();
    tauri::async_runtime::spawn(async move {
        let future = execute(&app, &action, String::new(), &run_id, &cancel, 0);
        let state = app.state::<AppState>();
        let result =
            with_workflow_deadline(&state, &run_id, future, std::time::Duration::from_secs(600))
                .await;
        if let Some(error) = action_failure(&action.title, result, cancel.load(Ordering::Relaxed)) {
            let _ = app.emit("action_error", error);
        }
        {
            let mut active = state.active.lock().unwrap();
            if active.get(&binding) == Some(&run_id) {
                active.remove(&binding);
            }
        }
        state.cancellations.lock().unwrap().remove(&run_id);
    });
    id
}

async fn with_workflow_deadline(
    state: &AppState,
    run: &str,
    future: impl Future<Output = Result<String, String>>,
    duration: std::time::Duration,
) -> Result<String, String> {
    match tokio::time::timeout(duration, future).await {
        Ok(result) => result,
        Err(_) => {
            // Dropping the process future stops its child, bypassing the normal
            // agent_launch completion update. Only expire this run's live launch;
            // a completed launch followed by a later slow step must stay exited.
            if let Some(session) = state.hook_agents.lock().unwrap().get_mut(run) {
                if session.source == "openstreamdeck" && session.raw_status == "process-running" {
                    session.status = "failed".into();
                    session.raw_status = "workflow-timeout".into();
                    session.updated_at = now();
                }
            }
            Err("Workflow timed out after 10 minutes".into())
        }
    }
}
// Consume completed results immediately; only a current failure is sent to the UI.
fn action_failure(title: &str, result: Result<String, String>, cancelled: bool) -> Option<String> {
    if cancelled {
        None
    } else {
        result.err().map(|error| format!("{title}: {error}"))
    }
}

struct NativeHost<'a> {
    app: &'a tauri::AppHandle,
    run: &'a str,
}
impl crate::engine::Host for NativeHost<'_> {
    fn config(&self) -> Config {
        self.app.state::<AppState>().config.lock().unwrap().clone()
    }
    fn next_cycle(&self, key: &str, len: usize) -> usize {
        let state = self.app.state::<AppState>();
        let binding = state
            .active
            .lock()
            .unwrap()
            .iter()
            .find(|(_, id)| *id == self.run)
            .map(|(k, _)| k.clone())
            .unwrap_or_default();
        let mut cycles = state.cycles.lock().unwrap();
        let n = cycles.entry(format!("{binding}:{key}")).or_default();
        let i = *n % len;
        *n += 1;
        i
    }
    fn effect<'a>(
        &'a self,
        a: &'a Action,
        input: String,
        cancel: &'a Arc<AtomicBool>,
    ) -> crate::engine::Outcome<'a> {
        perform_effect(self.app, a, input, self.run, cancel, 0)
    }
}
pub fn execute<'a>(
    app: &'a tauri::AppHandle,
    a: &'a Action,
    input: String,
    run: &'a str,
    cancel: &'a Arc<AtomicBool>,
    depth: usize,
) -> crate::engine::Outcome<'a> {
    Box::pin(async move {
        crate::engine::execute(&NativeHost { app, run }, a, input, cancel, depth).await
    })
}
pub fn validate_shortcut(value: &str) -> Result<(), String> {
    for part in value.split('+') {
        key(part.trim())?;
    }
    Ok(())
}
fn perform_effect<'a>(
    app: &'a tauri::AppHandle,
    a: &'a Action,
    input: String,
    run: &'a str,
    cancel: &'a Arc<AtomicBool>,
    depth: usize,
) -> Pin<Box<dyn Future<Output = Result<String, String>> + Send + 'a>> {
    Box::pin(async move {
        if depth > 32 {
            return Err("Action nesting exceeds 32 levels".into());
        }
        if cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let value = a.value.clone();
        let state = app.state::<AppState>();
        match a.kind.as_str(){
        "none"=>Ok(input),
        "open"=>{let value=value.clone();tokio::task::spawn_blocking(move||{
            if let Some((program,args))=crate::applications::launch_command(&value,std::env::consts::OS){let r=std::process::Command::new(program).args(args).status().map_err(|e|e.to_string())?;return if r.success(){Ok(())}else{Err("Application could not be opened".into())}}
            open::that(value).map_err(|e|e.to_string())
        }).await.map_err(|e|e.to_string())??;Ok(input)},
        "shortcut"|"text"|"media"|"mouse"=>{let a=a.clone();tokio::task::spawn_blocking(move||input_action(&a,&value)).await.map_err(|e|e.to_string())??;Ok(input)},
        "shell"=>{#[cfg(windows)]let(program,args)=("cmd",vec!["/C".to_string(),value]);#[cfg(not(windows))]let(program,args)=("/bin/sh",vec!["-c".to_string(),value]);process_with_input(program,&args,a.options.get("cwd").map(String::as_str),cancel,Some(&input)).await},
        "clipboard_read"=>tokio::task::spawn_blocking(||arboard::Clipboard::new().and_then(|mut c|c.get_text()).map_err(|e|e.to_string())).await.map_err(|e|e.to_string())?,
        "clipboard_write"=>{let text=value.clone();tokio::task::spawn_blocking(move||arboard::Clipboard::new().and_then(|mut c|c.set_text(text)).map_err(|e|e.to_string())).await.map_err(|e|e.to_string())??;Ok(value)},
        "file_read"=>read_text_file(&value).await,
        "file_write"=>{write_text_file(&value,&input).await?;Ok(input)},
        "ai"=>{let p=state.config.lock().unwrap().providers.iter().find(|p|Some(&p.id)==a.options.get("provider")).cloned().ok_or("Select a provider for this step")?;crate::ai::generate(&p,&value,cancel).await},
        "brightness"=>{let n={let mut w=state.config.lock().unwrap();crate::store::update(&state.dir,&mut w,|next|{next.brightness=crate::engine::brightness(next.brightness,&value)?;Ok(())})?;let _=app.emit("config",w.clone());w.brightness};state.send(DeviceCommand::Brightness(n))?;Ok(input)},
        "sleep"=>{state.send(DeviceCommand::ToggleSleep)?;Ok(input)},
        "wake"=>{state.send(DeviceCommand::Sleep(false))?;Ok(input)},
        "page"=>{state.agent_display.store(false,Ordering::Relaxed);let mut w=state.config.lock().unwrap();crate::store::update(&state.dir,&mut w,|next|crate::engine::switch_page(next,&value))?;let _=app.emit("config",w.clone());state.send(DeviceCommand::Redraw)?;Ok(input)},
        "agents"=>{crate::navigation::dashboard(&state,&crate::navigation::NativeWindow(app))?;Ok(input)},
        "agent_select"=>{let agents=state.agents.lock().unwrap();let count=agents.len();if count>0{let mut n=state.selected_agent.lock().unwrap();*n=if value=="previous"{(*n+count-1)%count}else{(*n+1)%count};let _=app.emit("agent-selected",*n);}Ok(input)},
        "agent_focus"|"agent_prompt"=>{let s={let agents=state.agents.lock().unwrap();let n=*state.selected_agent.lock().unwrap();agents.get(n).cloned().ok_or("No agent selected")?};if a.kind=="agent_focus"{crate::agents::focus(&s).await?}else{crate::agents::prompt(&s,&value).await?}Ok(input)},
        "agent_launch"=>{
            let agent=a.options.get("agent").map(String::as_str).unwrap_or("codex");
            if !["codex","claude"].contains(&agent){return Err("Choose Codex or Claude Code".into())}
            let args=if agent=="claude"{vec!["-p".into(),"--output-format".into(),"stream-json".into(),"--verbose".into(),value]}else{vec!["exec".into(),"--json".into(),value]};
            let session=AgentSession{id:format!("launch:{run}"),source:"openstreamdeck".into(),agent:agent.into(),title:a.title.clone(),status:"working".into(),raw_status:"process-running".into(),session_id:None,target:run.into(),updated_at:now(),parent_session_id:None,is_subagent:false,can_focus:false,can_prompt:false};
            state.hook_agents.lock().unwrap().insert(run.into(),session);
            let result=process(&crate::agents::binary(agent),&args,a.options.get("cwd").map(String::as_str),cancel).await;
            if let Some(session)=state.hook_agents.lock().unwrap().get_mut(run){session.status=if cancel.load(Ordering::Relaxed){"cancelled"}else if result.is_ok(){"exited"}else{"failed"}.into();session.raw_status="process-exited".into();session.updated_at=now();}
            result
        },
        "super_hotkey"=>Err("Hardware keyboard packet sequencing is not yet hardware-verified. Use a software shortcut.".into()),
        _=>Err(format!("Unsupported action: {}",a.kind))
    }
    })
}
async fn read_text_file(path: &str) -> Result<String, String> {
    let file = tokio::fs::File::open(path)
        .await
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(2_000_001)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() > 2_000_000 {
        return Err("Text input exceeds 2 MB".into());
    }
    String::from_utf8(bytes).map_err(|_| "Text input is not valid UTF-8".into())
}
async fn write_text_file(path: &str, input: &str) -> Result<(), String> {
    tokio::fs::write(path, input.as_bytes())
        .await
        .map_err(|e| e.to_string())
}
/// Drain pipes continuously while retaining a bounded result. Never stop reading
/// just because the output budget has been reached (that can deadlock a child).
async fn drain<R: tokio::io::AsyncRead + Unpin>(
    mut reader: R,
    limit: usize,
) -> Result<(Vec<u8>, bool), String> {
    let mut kept = Vec::new();
    let mut buffer = [0u8; 8192];
    let mut truncated = false;
    loop {
        let n = reader.read(&mut buffer).await.map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        let remaining = limit.saturating_sub(kept.len());
        kept.extend_from_slice(&buffer[..n.min(remaining)]);
        truncated |= n > remaining;
    }
    Ok((kept, truncated))
}
#[cfg(unix)]
struct ProcessGroup(u32);
#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        unsafe {
            libc::kill(-(self.0 as i32), libc::SIGKILL);
        }
    }
}
async fn cancelled(cancel: &AtomicBool) {
    while !cancel.load(Ordering::Relaxed) {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}
pub async fn process(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    process_with_input(program, args, cwd, cancel, None).await
}
async fn process_with_input(
    program: &str,
    args: &[String],
    cwd: Option<&str>,
    cancel: &Arc<AtomicBool>,
    input: Option<&str>,
) -> Result<String, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Cancelled".into());
    }
    let mut command = tokio::process::Command::new(program);
    if let Some(input) = input {
        command.env("OPENSTREAMDECK_INPUT", input);
    }
    command
        .args(args)
        .kill_on_drop(true)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    if let Some(dir) = cwd.filter(|s| !s.is_empty()) {
        command.current_dir(dir);
    }
    #[cfg(windows)]
    crate::process_tree::prepare(&mut command);
    let mut child = command.spawn().map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let _tree = crate::process_tree::ProcessTree::attach(&child)?;
    #[cfg(unix)]
    let _group = ProcessGroup(child.id().ok_or("Child has no PID")?);
    let out = child.stdout.take().ok_or("Missing stdout")?;
    let err = child.stderr.take().ok_or("Missing stderr")?;
    let completion = async {
        tokio::try_join!(drain(out, 2_000_000), drain(err, 200_000), async {
            child.wait().await.map_err(|e| e.to_string())
        })
    };
    let ((bytes, truncated), (error, _), status) = tokio::select! {result=completion=>result?,_=cancelled(cancel)=>return Err("Cancelled".into())};
    if !status.success() {
        return Err(format!(
            "Process exited {status}: {}",
            String::from_utf8_lossy(&error)
        ));
    }
    let mut text = String::from_utf8_lossy(&bytes).into_owned();
    if truncated {
        text.push_str("\n[Output truncated at 2 MB]")
    }
    Ok(text)
}

fn key(value: &str) -> Result<Key, String> {
    Ok(match value.to_lowercase().as_str() {
        "meta" | "cmd" | "super" => Key::Meta,
        "ctrl" | "control" => Key::Control,
        "shift" => Key::Shift,
        "alt" | "option" => Key::Alt,
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "space" => Key::Space,
        "backspace" => Key::Backspace,
        "delete" => Key::Delete,
        "left" => Key::LeftArrow,
        "right" => Key::RightArrow,
        "up" => Key::UpArrow,
        "down" => Key::DownArrow,
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,
        s if s.chars().count() == 1 => Key::Unicode(s.chars().next().unwrap()),
        _ => return Err(format!("Unknown key: {value}")),
    })
}
trait InputDevice {
    fn text(&mut self, text: &str) -> Result<(), String>;
    fn key(&mut self, key: Key, direction: Direction) -> Result<(), String>;
    fn button(&mut self, button: enigo::Button) -> Result<(), String>;
}
impl InputDevice for Enigo {
    fn text(&mut self, text: &str) -> Result<(), String> {
        Keyboard::text(self, text).map_err(|e| e.to_string())
    }
    fn key(&mut self, key: Key, direction: Direction) -> Result<(), String> {
        Keyboard::key(self, key, direction).map_err(|e| e.to_string())
    }
    fn button(&mut self, button: enigo::Button) -> Result<(), String> {
        Mouse::button(self, button, Direction::Click).map_err(|e| e.to_string())
    }
}
fn input_action(a: &Action, value: &str) -> Result<(), String> {
    let mut device = Enigo::new(&Settings::default()).map_err(|e| e.to_string())?;
    send_input(&mut device, &a.kind, value)
}
/// The same dispatch is exercised with a recording backend; tests never create
/// an OS input connection or type into the user's active window.
fn send_input(device: &mut dyn InputDevice, kind: &str, value: &str) -> Result<(), String> {
    match kind {
        "text" => device.text(value),
        "media" => {
            let k = match value {
                "play" => Key::MediaPlayPause,
                "next" => Key::MediaNextTrack,
                "previous" => Key::MediaPrevTrack,
                "volume_up" => Key::VolumeUp,
                "volume_down" => Key::VolumeDown,
                "mute" => Key::VolumeMute,
                _ => return Err("Unknown media control".into()),
            };
            device.key(k, Direction::Click)
        }
        "mouse" => device.button(match value {
            "left" => enigo::Button::Left,
            "right" => enigo::Button::Right,
            "middle" => enigo::Button::Middle,
            _ => return Err("Unknown mouse button".into()),
        }),
        "shortcut" => {
            let keys = value
                .split('+')
                .map(|s| key(s.trim()))
                .collect::<Result<Vec<_>, _>>()?;
            let mut pressed = vec![];
            let mut result = Ok(());
            for k in keys {
                match device.key(k, Direction::Press) {
                    Ok(()) => pressed.push(k),
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
            for k in pressed.into_iter().rev() {
                if let Err(e) = device.key(k, Direction::Release) {
                    result = Err(e);
                }
            }
            result
        }
        _ => Err("Unsupported input action".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launch_fixture(run: &str) -> AgentSession {
        AgentSession {
            id: format!("launch:{run}"),
            source: "openstreamdeck".into(),
            agent: "fixture".into(),
            title: "Controlled process".into(),
            status: "working".into(),
            raw_status: "process-running".into(),
            session_id: None,
            target: run.into(),
            updated_at: 0,
            parent_session_id: None,
            is_subagent: false,
            can_focus: false,
            can_prompt: false,
        }
    }

    #[tokio::test]
    async fn deadline_only_expires_this_runs_active_launch() {
        let (tx, _) = std::sync::mpsc::sync_channel(1);
        let state = AppState::new(Default::default(), Config::default(), tx);
        for (source, raw, status) in [
            ("openstreamdeck", "process-running", "working"),
            ("openstreamdeck", "process-exited", "exited"),
            ("claude-hook", "working", "working"),
        ] {
            let mut session = launch_fixture("run");
            session.source = source.into();
            session.raw_status = raw.into();
            session.status = status.into();
            state
                .hook_agents
                .lock()
                .unwrap()
                .insert("run".into(), session);
            state
                .hook_agents
                .lock()
                .unwrap()
                .insert("other".into(), launch_fixture("other"));
            let result = with_workflow_deadline(
                &state,
                "run",
                std::future::pending(),
                std::time::Duration::from_millis(1),
            )
            .await;
            assert_eq!(result.unwrap_err(), "Workflow timed out after 10 minutes");
            let sessions = state.hook_agents.lock().unwrap();
            let session = &sessions["run"];
            if raw == "process-running" {
                assert_eq!(session.status, "failed");
                assert_eq!(session.raw_status, "workflow-timeout");
                assert!(session.updated_at > 0);
            } else {
                assert_eq!(session.status, status);
                assert_eq!(session.raw_status, raw);
                assert_eq!(session.updated_at, 0);
            }
            assert_eq!(sessions["other"].raw_status, "process-running");
        }
        for result in [Ok("output".into()), Err("process failed".into())] {
            assert_eq!(
                with_workflow_deadline(
                    &state,
                    "missing",
                    std::future::ready(result.clone()),
                    std::time::Duration::from_secs(1),
                )
                .await,
                result
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn launch_deadline_stops_controlled_process_and_marks_failure() {
        use std::time::Duration;
        let dir = tempfile::tempdir().unwrap();
        let (tx, _) = std::sync::mpsc::sync_channel(1);
        let state = AppState::new(dir.path().into(), Config::default(), tx);
        state
            .hook_agents
            .lock()
            .unwrap()
            .insert("run".into(), launch_fixture("run"));
        let args = [
            "-c".into(),
            "printf '%s' \"$$\" > child.pid; exec sleep 30".into(),
        ];
        let cancel = Arc::new(AtomicBool::new(false));
        let mut child = Box::pin(process("/bin/sh", &args, dir.path().to_str(), &cancel));
        // Wait for this fixture's ready file before applying the short deadline.
        let pid = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                tokio::select! {
                    result = &mut child => panic!("Fixture exited before deadline: {result:?}"),
                    _ = tokio::time::sleep(Duration::from_millis(10)) => {
                        if let Ok(text) = std::fs::read_to_string(dir.path().join("child.pid")) {
                            if let Ok(pid) = text.parse::<i32>() { break pid; }
                        }
                    }
                }
            }
        })
        .await
        .unwrap();
        assert!(
            with_workflow_deadline(&state, "run", child, Duration::from_millis(1))
                .await
                .is_err()
        );
        assert_eq!(
            state.hook_agents.lock().unwrap()["run"].raw_status,
            "workflow-timeout"
        );
        tokio::time::timeout(Duration::from_secs(5), async {
            while unsafe { libc::kill(pid, 0) } == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("Timed-out process must stop");
    }

    #[test]
    fn completion_discards_output_and_reports_only_uncancelled_failures() {
        assert_eq!(
            action_failure("AI", Ok("transient output".into()), false),
            None
        );
        assert_eq!(
            action_failure("Action", Err("device unavailable".into()), false),
            Some("Action: device unavailable".into())
        );
        assert_eq!(
            action_failure("Action", Err("Cancelled".into()), true),
            None
        );
    }

    #[tokio::test]
    async fn text_files_cover_success_invalid_encoding_size_and_io_failures() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("text.txt");
        let path = path.to_str().unwrap();
        assert!(read_text_file(path).await.is_err());
        write_text_file(path, "literal {{input}} 🦀").await.unwrap();
        assert_eq!(read_text_file(path).await.unwrap(), "literal {{input}} 🦀");
        tokio::fs::write(path, [0xff, 0xfe]).await.unwrap();
        assert!(read_text_file(path).await.unwrap_err().contains("UTF-8"));
        tokio::fs::write(path, vec![b'a'; 2_000_001]).await.unwrap();
        assert!(read_text_file(path).await.unwrap_err().contains("2 MB"));
        assert!(
            write_text_file(dir.path().to_str().unwrap(), "cannot overwrite directory")
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn process_missing_binary_and_precancel_are_explicit() {
        let cancel = Arc::new(AtomicBool::new(false));
        assert!(process(
            "__missing_openstreamdeck_fixture_executable__",
            &[],
            None,
            &cancel
        )
        .await
        .is_err());
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(
            process(
                "__missing_openstreamdeck_fixture_executable__",
                &[],
                None,
                &cancel
            )
            .await
            .unwrap_err(),
            "Cancelled"
        );
    }
    #[tokio::test]
    async fn pipe_budget_does_not_deadlock() {
        let (mut sender, reader) = tokio::io::duplex(32);
        let write = tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            sender.write_all(&vec![42; 100_000]).await.unwrap();
        });
        let (bytes, truncated) =
            tokio::time::timeout(std::time::Duration::from_secs(2), drain(reader, 100))
                .await
                .unwrap()
                .unwrap();
        write.await.unwrap();
        assert_eq!(bytes.len(), 100);
        assert!(truncated);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn shell_pipeline_input_is_literal_environment_data() {
        let dir = tempfile::tempdir().unwrap();
        let marker = dir.path().join("must-not-exist");
        let input = format!("$(touch '{}'); 'quoted' `false`", marker.display());
        let result = process_with_input(
            "/bin/sh",
            &["-c".into(), "printf '%s' \"$OPENSTREAMDECK_INPUT\"".into()],
            None,
            &Arc::new(AtomicBool::new(false)),
            Some(&input),
        )
        .await
        .unwrap();
        assert_eq!(result, input);
        assert!(!marker.exists());
    }
    #[cfg(windows)]
    const ENV_FIXTURE_INPUT: &str =
        "OPENSTREAMDECK_TEST_LITERAL: $(not-a-command) & echo untrusted | %PATH%";
    #[cfg(windows)]
    #[test]
    fn environment_reader_fixture() {
        // Explicit known-value gate: an ordinary test run does nothing, and
        // arbitrary inherited environment values are never logged.
        if std::env::var("OPENSTREAMDECK_INPUT").as_deref() == Ok(ENV_FIXTURE_INPUT) {
            println!("DATA={ENV_FIXTURE_INPUT}");
        }
    }
    #[cfg(windows)]
    #[tokio::test]
    async fn windows_pipeline_input_is_literal_environment_data() {
        let exe = std::env::current_exe().unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            process_with_input(
                exe.to_str().unwrap(),
                &[
                    "--exact".into(),
                    "actions::tests::environment_reader_fixture".into(),
                    "--nocapture".into(),
                ],
                None,
                &Arc::new(AtomicBool::new(false)),
                Some(ENV_FIXTURE_INPUT),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(result.contains(&format!("DATA={ENV_FIXTURE_INPUT}")));
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn command_exit_and_cancellation() {
        let cancel = Arc::new(AtomicBool::new(false));
        assert_eq!(
            process(
                "/bin/sh",
                &["-c".into(), "printf hello".into()],
                None,
                &cancel
            )
            .await
            .unwrap(),
            "hello"
        );
        assert!(process(
            "/bin/sh",
            &["-c".into(), "printf failure >&2; exit 7".into()],
            None,
            &cancel
        )
        .await
        .unwrap_err()
        .contains("failure"));
        let signal = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            signal.store(true, Ordering::Relaxed);
        });
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            process("/bin/sh", &["-c".into(), "sleep 30".into()], None, &cancel),
        )
        .await
        .unwrap();
        assert_eq!(result.unwrap_err(), "Cancelled");
    }
}

#[cfg(test)]
mod input_tests {
    use super::*;
    #[derive(Default)]
    struct RecordingInput {
        calls: Vec<String>,
        fail_at: Option<usize>,
    }
    impl RecordingInput {
        fn record(&mut self, value: String) -> Result<(), String> {
            self.calls.push(value);
            if self.fail_at == Some(self.calls.len()) {
                Err("Injected OS input failure".into())
            } else {
                Ok(())
            }
        }
    }
    impl InputDevice for RecordingInput {
        fn text(&mut self, value: &str) -> Result<(), String> {
            self.record(format!("text:{value}"))
        }
        fn key(&mut self, key: Key, direction: Direction) -> Result<(), String> {
            self.record(format!("{key:?}:{direction:?}"))
        }
        fn button(&mut self, button: enigo::Button) -> Result<(), String> {
            self.record(format!("button:{button:?}"))
        }
    }
    #[test]
    fn every_media_and_mouse_action_dispatches_and_propagates_os_failure() {
        for (value, expected) in [
            ("play", Key::MediaPlayPause),
            ("next", Key::MediaNextTrack),
            ("previous", Key::MediaPrevTrack),
            ("volume_up", Key::VolumeUp),
            ("volume_down", Key::VolumeDown),
            ("mute", Key::VolumeMute),
        ] {
            let mut input = RecordingInput::default();
            send_input(&mut input, "media", value).unwrap();
            assert_eq!(input.calls, [format!("{expected:?}:Click")]);
            input.fail_at = Some(2);
            assert!(send_input(&mut input, "media", value).is_err());
        }
        for (value, expected) in [
            ("left", enigo::Button::Left),
            ("middle", enigo::Button::Middle),
            ("right", enigo::Button::Right),
        ] {
            let mut input = RecordingInput::default();
            send_input(&mut input, "mouse", value).unwrap();
            assert_eq!(input.calls, [format!("button:{expected:?}")]);
            input.fail_at = Some(2);
            assert!(send_input(&mut input, "mouse", value).is_err());
        }
    }
    #[test]
    fn shortcuts_release_modifiers_on_press_and_release_failures() {
        let mut input = RecordingInput {
            fail_at: Some(3),
            ..Default::default()
        };
        assert!(send_input(&mut input, "shortcut", "meta+shift+c").is_err());
        assert_eq!(
            input.calls,
            [
                "Meta:Press",
                "Shift:Press",
                "Unicode('c'):Press",
                "Shift:Release",
                "Meta:Release"
            ]
        );
        let mut input = RecordingInput {
            fail_at: Some(4),
            ..Default::default()
        };
        assert!(send_input(&mut input, "shortcut", "meta+shift+c").is_err());
        assert_eq!(input.calls.last().unwrap(), "Meta:Release");
        let mut input = RecordingInput::default();
        send_input(&mut input, "shortcut", "ctrl+alt+f12").unwrap();
        assert_eq!(input.calls.last().unwrap(), "Control:Release");
    }
    #[test]
    fn text_is_literal_and_invalid_input_never_reaches_backend() {
        let mut input = RecordingInput::default();
        send_input(&mut input, "text", "hello 🦀 + {{input}}").unwrap();
        assert_eq!(input.calls, ["text:hello 🦀 + {{input}}"]);
        input.fail_at = Some(2);
        assert!(send_input(&mut input, "text", "failure").is_err());
        for (kind, value) in [
            ("shortcut", "meta+unknown-key"),
            ("media", "bad"),
            ("mouse", "bad"),
            ("unsupported", ""),
        ] {
            let mut input = RecordingInput::default();
            assert!(send_input(&mut input, kind, value).is_err());
            assert!(input.calls.is_empty());
        }
    }
}
