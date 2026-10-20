use crate::{model::*, AppState};
use serde_json::{json, Value};
use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};

pub fn binary(name: &str) -> String {
    for dir in ["/opt/homebrew/bin", "/usr/local/bin"] {
        let p = format!("{dir}/{name}");
        if std::path::Path::new(&p).exists() {
            return p;
        }
    }
    name.into()
}
pub async fn output(program: &str, args: &[&str]) -> Result<String, String> {
    let mut c = tokio::process::Command::new(binary(program));
    c.args(args).kill_on_drop(true);
    let o = tokio::time::timeout(std::time::Duration::from_secs(4), c.output())
        .await
        .map_err(|_| "Command timed out")?
        .map_err(|e| e.to_string())?;
    if !o.status.success() {
        return Err(String::from_utf8_lossy(&o.stderr)
            .chars()
            .take(400)
            .collect());
    }
    Ok(String::from_utf8_lossy(&o.stdout).into())
}
pub fn normalize(s: &str) -> &str {
    match s {
        "working" | "active" | "inProgress" => "working",
        "blocked" | "waitingOnApproval" | "waitingOnUserInput" => "attention",
        "done" | "completed" | "idle" => "ready",
        "failed" | "error" => "failed",
        "interrupted" => "cancelled",
        "process-exited" => "exited",
        "disconnected" => "disconnected",
        _ => "unknown",
    }
}
pub fn parse_herdr(v: &Value, session: &str) -> Vec<AgentSession> {
    v["result"]["snapshot"]["agents"]
        .as_array()
        .or_else(|| v["result"]["agents"].as_array())
        .map(|arr| {
            arr.iter()
                .map(|a| {
                    let target = a["pane_id"].as_str().unwrap_or_default();
                    let raw = a["agent_status"].as_str().unwrap_or("unknown");
                    AgentSession {
                        id: format!("herdr:{session}:{target}"),
                        source: format!("herdr:{session}"),
                        agent: a["agent"]
                            .as_str()
                            .or(a["display_agent"].as_str())
                            .unwrap_or("terminal")
                            .into(),
                        title: a["name"]
                            .as_str()
                            .or(a["title"].as_str())
                            .or(a["terminal_title_stripped"].as_str())
                            .unwrap_or("Agent session")
                            .into(),
                        status: normalize(raw).into(),
                        raw_status: raw.into(),
                        session_id: a["agent_session"]["value"].as_str().map(str::to_owned),
                        target: target.into(),
                        updated_at: now(),
                        parent_session_id: None,
                        is_subagent: false,
                        can_focus: !target.is_empty(),
                        can_prompt: !target.is_empty(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}
pub async fn herdr_sessions() -> Result<Vec<AgentSession>, String> {
    let list: Value = serde_json::from_str(&output("herdr", &["session", "list", "--json"]).await?)
        .map_err(|e| e.to_string())?;
    let mut agents = vec![];
    if let Some(sessions) = list["sessions"].as_array() {
        for s in sessions {
            if s["running"] == true {
                let name = s["name"].as_str().unwrap_or("default");
                let raw = output("herdr", &["--session", name, "api", "snapshot"]).await?;
                let v = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
                agents.extend(parse_herdr(&v, name));
            }
        }
    }
    Ok(agents)
}
pub fn focus_args(session: &AgentSession) -> Result<Vec<String>, String> {
    let name = session
        .source
        .strip_prefix("herdr:")
        .ok_or("Not a Herdr session")?;
    if name.is_empty() || session.target.is_empty() {
        return Err("Missing Herdr session or pane ID".into());
    }
    Ok(vec![
        "--session".into(),
        name.into(),
        "agent".into(),
        "focus".into(),
        session.target.clone(),
    ])
}
pub async fn focus(session: &AgentSession) -> Result<(), String> {
    if !session.can_focus {
        return Err("This session has no available focus adapter".into());
    }
    if session.source.starts_with("herdr:") {
        let args = focus_args(session)?;
        output(
            "herdr",
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
        )
        .await?;
        crate::focus::herdr_terminal().await
    } else if session.source == "codex" {
        crate::focus::codex(session).await
    } else {
        Err("This adapter cannot focus the originating window".into())
    }
}

pub async fn prompt(session: &AgentSession, text: &str) -> Result<(), String> {
    if !session.can_prompt || text.trim().is_empty() {
        return Err("Prompt routing is unavailable or the prompt is empty".into());
    }
    if let Some(name) = session.source.strip_prefix("herdr:") {
        output(
            "herdr",
            &["--session", name, "agent", "prompt", &session.target, text],
        )
        .await?;
        Ok(())
    } else {
        Err("Prompt routing is unavailable for this session".into())
    }
}

pub async fn monitor(app: tauri::AppHandle) {
    loop {
        let mut errors = vec![];
        let mut list = match herdr_sessions().await {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("Herdr: {e}"));
                vec![]
            }
        };
        match crate::codex_adapter::sessions().await {
            Ok(v) => {
                for s in v {
                    if !list
                        .iter()
                        .any(|a| a.session_id.is_some() && a.session_id == s.session_id)
                    {
                        list.push(s)
                    }
                }
            }
            Err(e) => errors.push(format!("Codex: {e}")),
        }
        let state = app.state::<AppState>();
        {
            let hooks = state.hook_agents.lock().unwrap();
            for s in hooks.values() {
                let mut s = s.clone();
                if now().saturating_sub(s.updated_at) > 600_000 {
                    s.status = "unknown".into()
                }
                if !list
                    .iter()
                    .any(|a| a.session_id.is_some() && a.session_id == s.session_id)
                {
                    list.push(s)
                }
            }
        }
        {
            let mut agents = state.agents.lock().unwrap();
            let mut selected = state.selected_agent.lock().unwrap();
            *selected = reconcile_selection(&agents, &list, *selected);
            *agents = list.clone();
        }
        *state.adapter_errors.lock().unwrap() = errors;
        let _ = app.emit("agents", &list);
        let overlay = state.page_context.lock().unwrap().overlay;
        if overlay {
            display(&app, &list);
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
}
pub fn reconcile_selection(
    previous: &[AgentSession],
    next: &[AgentSession],
    index: usize,
) -> usize {
    previous
        .get(index)
        .and_then(|selected| next.iter().position(|a| a.id == selected.id))
        .unwrap_or_else(|| index.min(next.len().saturating_sub(1)))
}
pub fn display(app: &tauri::AppHandle, sessions: &[AgentSession]) {
    let state = app.state::<AppState>();
    let device = state.device.lock().unwrap().clone();
    if !device.connected || device.sleeping {
        return;
    }
    let (w, epoch) = {
        let config = state.config.lock().unwrap();
        let context = state.page_context.lock().unwrap();
        (config.clone(), context.overlay_epoch)
    };
    let selected = *state.selected_agent.lock().unwrap();
    let page = selected / 6;
    let fingerprint = serde_json::to_string(&(
        epoch,
        w.image_size,
        w.rotation,
        sessions
            .iter()
            .skip(page * 6)
            .take(6)
            .map(|s| (&s.id, &s.title, &s.status))
            .collect::<Vec<_>>(),
    ))
    .unwrap_or_default();
    {
        let old = state.agent_frame.lock().unwrap();
        if *old == fingerprint {
            return;
        }
    }
    let mut images = Vec::with_capacity(6);
    for i in 0..6 {
        let a = if let Some(s) = sessions.get(page * 6 + i) {
            Action {
                kind: "agents".into(),
                title: format!("{}\n{}", s.agent, s.status),
                color: match s.status.as_str() {
                    "working" => "#315c67",
                    "attention" => "#9a6326",
                    "ready" => "#2b644a",
                    "failed" => "#873b46",
                    _ => "#343746",
                }
                .into(),
                ..Default::default()
            }
        } else {
            Action {
                title: "No session".into(),
                ..Default::default()
            }
        };
        match crate::device::render(&a, w.image_size, w.rotation) {
            Ok(bytes) => {
                images.push(bytes);
            }
            Err(error) => {
                let _ = app.emit("action_error", format!("Agent artwork: {error}"));
                return;
            }
        }
    }
    // Only the worker caches successfully transmitted artwork. Enqueue success
    // is not display success: sleep, mode changes or disconnect may reject it.
    let _ = state.tx.try_send(crate::device::DeviceCommand::AgentFrame(
        epoch,
        images,
        sessions.iter().skip(page * 6).take(6).cloned().collect(),
        fingerprint,
    ));
}
/// Loopback hook receiver. Token is generated at startup and stored only in local app data.
pub async fn hook_server(app: tauri::AppHandle) {
    let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
        Ok(v) => v,
        Err(_) => return,
    };
    let token = uuid::Uuid::new_v4().to_string();
    let state = app.state::<AppState>();
    let config = json!({"port":listener.local_addr().unwrap().port(),"token":token});
    let p = state.dir.join("agent-bridge.json");
    if write_bridge_config(&p, &config.to_string()).is_err() {
        return;
    }
    let connections = std::sync::Arc::new(tokio::sync::Semaphore::new(32));
    loop {
        let Ok((socket, _)) = listener.accept().await else {
            continue;
        };
        let Ok(permit) = connections.clone().try_acquire_owned() else {
            continue;
        };
        let app = app.clone();
        let token = token.clone();
        tokio::spawn(async move {
            let _permit = permit;
            let work = async {
                let mut reader = BufReader::new(socket.take(65537));
                let mut line = String::new();
                if reader.read_line(&mut line).await.unwrap_or(0) > 65536 {
                    return;
                }
                let v: Value = match serde_json::from_str(&line) {
                    Ok(v) => v,
                    Err(_) => return,
                };
                if v["token"] != token {
                    return;
                }
                let data = &v["event"];
                let id = data["session_id"].as_str().unwrap_or_default();
                if id.is_empty() {
                    return;
                }
                let event = data["hook_event_name"].as_str().unwrap_or_default();
                let raw = match event {
                    "SessionStart" => "idle",
                    "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => "working",
                    "PermissionRequest" => "blocked",
                    "Notification" if data["notification_type"] == "permission_prompt" => "blocked",
                    "Stop" => "done",
                    "StopFailure" => "failed",
                    "SessionEnd" => "disconnected",
                    _ => "unknown",
                };
                let state = app.state::<AppState>();
                let mut agents = state.hook_agents.lock().unwrap();
                agents.insert(
                    id.into(),
                    AgentSession {
                        id: format!("claude:{id}"),
                        source: "claude-hook".into(),
                        agent: "claude".into(),
                        title: data["cwd"]
                            .as_str()
                            .and_then(|s| std::path::Path::new(s).file_name())
                            .and_then(|s| s.to_str())
                            .unwrap_or("Claude Code")
                            .into(),
                        status: if raw == "disconnected" {
                            raw.into()
                        } else {
                            normalize(raw).into()
                        },
                        raw_status: raw.into(),
                        session_id: Some(id.into()),
                        target: id.into(),
                        updated_at: now(),
                        parent_session_id: None,
                        is_subagent: false,
                        can_focus: false,
                        can_prompt: false,
                    },
                );
            };
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), work).await;
        });
    }
}
/// Set permissions before writing any token bytes, including an existing file.
fn write_bridge_config(path: &std::path::Path, config: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(config.as_bytes())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_configuration_is_private_before_token_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bridge.json");
        write_bridge_config(&path, "test-token").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "test-token");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn herdr_snapshot() {
        let v = json!({"result":{"snapshot":{"agents":[{"pane_id":"p1","agent":"codex","agent_status":"blocked","agent_session":{"value":"thread1"}}]}}});
        let a = parse_herdr(&v, "work");
        assert_eq!(a[0].status, "attention");
        assert_eq!(a[0].session_id.as_deref(), Some("thread1"));
        assert_eq!(normalize("nonsense"), "unknown");
    }
    #[test]
    fn turns_are_activity_not_task_completion() {
        for (raw, expected) in [
            ("working", "working"),
            ("active", "working"),
            ("inProgress", "working"),
            ("done", "ready"),
            ("completed", "ready"),
            ("idle", "ready"),
            ("blocked", "attention"),
            ("waitingOnApproval", "attention"),
            ("waitingOnUserInput", "attention"),
            ("failed", "failed"),
            ("error", "failed"),
            ("interrupted", "cancelled"),
            ("process-exited", "exited"),
            ("disconnected", "disconnected"),
            ("future-status", "unknown"),
        ] {
            assert_eq!(normalize(raw), expected);
        }
        let states: Vec<_> = ["working", "done", "working"]
            .map(normalize)
            .into_iter()
            .collect();
        assert_eq!(states, ["working", "ready", "working"]);
    }
    #[test]
    fn focus_preserves_opaque_pane_and_session_targets() {
        let v = json!({"result":{"agents":[{"pane_id":"wE:p2","agent_status":"done"}]}});
        let mut session = parse_herdr(&v, "work space").remove(0);
        assert_eq!(
            focus_args(&session).unwrap(),
            ["--session", "work space", "agent", "focus", "wE:p2"]
        );
        assert_eq!(session.status, "ready");
        session.target.clear();
        assert!(focus_args(&session).is_err());
        let missing = parse_herdr(&json!({"result":{"agents":[{}]}}), "work");
        assert!(!missing[0].can_focus);
        assert!(!missing[0].can_prompt);
    }
    #[test]
    fn new_children_do_not_change_the_selected_agent() {
        let old = parse_herdr(
            &json!({"result":{"agents":[{"pane_id":"parent"},{"pane_id":"child"}]}}),
            "s",
        );
        let new = parse_herdr(
            &json!({"result":{"agents":[{"pane_id":"new"},{"pane_id":"parent"},{"pane_id":"child"}]}}),
            "s",
        );
        assert_eq!(reconcile_selection(&old, &new, 1), 2);
        assert_eq!(reconcile_selection(&old, &[], 1), 0);
        assert_eq!(reconcile_selection(&old, &new[..1], 1), 0);
    }
    #[tokio::test]
    async fn unavailable_focus_and_prompt_fail_without_launching_anything() {
        let s = parse_herdr(&json!({"result":{"agents":[{}]}}), "work").remove(0);
        assert!(focus(&s).await.is_err());
        assert!(prompt(&s, "message").await.is_err());
    }
}
