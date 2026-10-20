use crate::model::*;
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use std::{
    pin::Pin,
    task::{Context, Poll},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

// `codex app-server proxy` forwards raw WebSocket bytes, not JSON lines.
struct ProxyStream {
    input: tokio::process::ChildStdin,
    output: tokio::process::ChildStdout,
}
impl AsyncRead for ProxyStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.output).poll_read(cx, buf)
    }
}
impl AsyncWrite for ProxyStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        Pin::new(&mut self.input).poll_write(cx, buf)
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.input).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.input).poll_shutdown(cx)
    }
}
async fn response<S: AsyncRead + AsyncWrite + Unpin>(
    socket: &mut WebSocketStream<S>,
    id: u64,
) -> Result<Value, String> {
    while let Some(message) = socket.next().await {
        let message = message.map_err(|e| e.to_string())?;
        if let Message::Text(text) = message {
            let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
            if value["id"] == id {
                if let Some(error) = value.get("error") {
                    return Err(format!("Codex rejected request: {}", error["message"]));
                }
                return Ok(value["result"].clone());
            }
        }
    }
    Err("Codex app-server closed its connection".into())
}
async fn snapshot<S: AsyncRead + AsyncWrite + Unpin>(
    socket: &mut WebSocketStream<S>,
) -> Result<Vec<AgentSession>, String> {
    socket.send(Message::Text(json!({"id":1,"method":"initialize","params":{"clientInfo":{"name":"openstreamdeck","version":env!("CARGO_PKG_VERSION")},"capabilities":{"experimentalApi":true}}}).to_string().into())).await.map_err(|e|e.to_string())?;
    response(socket, 1).await?;
    socket
        .send(Message::Text(
            json!({"method":"initialized"}).to_string().into(),
        ))
        .await
        .map_err(|e| e.to_string())?;
    // The default thread/list excludes children and stops at recent persisted
    // threads. Enumerate the live registry instead, then read metadata only.
    let mut cursor = None::<String>;
    let mut seen_cursors = std::collections::HashSet::new();
    let mut ids = std::collections::BTreeSet::new();
    let mut request_id = 2;
    loop {
        socket.send(Message::Text(json!({"id":request_id,"method":"thread/loaded/list","params":{"limit":100,"cursor":cursor}}).to_string().into())).await.map_err(|e|e.to_string())?;
        let result = response(socket, request_id).await?;
        request_id += 1;
        let data = result["data"]
            .as_array()
            .ok_or("Invalid Codex loaded-thread response")?;
        for id in data {
            ids.insert(
                id.as_str()
                    .ok_or("Invalid Codex loaded-thread ID")?
                    .to_owned(),
            );
        }
        cursor = result["nextCursor"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(str::to_owned);
        match &cursor {
            None => break,
            Some(c) if !seen_cursors.insert(c.clone()) => {
                return Err("Codex repeated its pagination cursor".into())
            }
            _ => {}
        }
    }
    let mut threads = vec![];
    for id in ids {
        socket.send(Message::Text(json!({"id":request_id,"method":"thread/read","params":{"threadId":id,"includeTurns":false}}).to_string().into())).await.map_err(|e|e.to_string())?;
        let result = response(socket, request_id).await?;
        request_id += 1;
        threads.push(result["thread"].clone());
    }
    // A child can unload while its parent's task is still ongoing. Include
    // those descendants without scanning unrelated history or reading turns.
    let roots: Vec<String> = threads
        .iter()
        .filter(|t| t["parentThreadId"].as_str().is_none() && t["source"].get("subAgent").is_none())
        .filter_map(|t| t["id"].as_str().map(str::to_owned))
        .collect();
    let mut known: std::collections::HashSet<String> = threads
        .iter()
        .filter_map(|t| t["id"].as_str().map(str::to_owned))
        .collect();
    for root in roots {
        let mut cursor = None::<String>;
        let mut cursors = std::collections::HashSet::new();
        loop {
            socket.send(Message::Text(json!({"id":request_id,"method":"thread/list","params":{"ancestorThreadId":root,"sourceKinds":["subAgent","subAgentReview","subAgentCompact","subAgentThreadSpawn","subAgentOther"],"limit":100,"cursor":cursor,"useStateDbOnly":true}}).to_string().into())).await.map_err(|e|e.to_string())?;
            let result = response(socket, request_id).await?;
            request_id += 1;
            for thread in result["data"]
                .as_array()
                .ok_or("Invalid Codex descendant response")?
            {
                if let Some(id) = thread["id"].as_str() {
                    if known.insert(id.to_owned()) {
                        threads.push(thread.clone());
                    }
                }
            }
            cursor = result["nextCursor"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_owned);
            match &cursor {
                None => break,
                Some(c) if !cursors.insert(c.clone()) => {
                    return Err("Codex repeated its descendant pagination cursor".into())
                }
                _ => {}
            }
        }
    }
    Ok(parse_threads(&json!({"data":threads})))
}

pub fn parse_threads(value: &Value) -> Vec<AgentSession> {
    value["data"]
        .as_array()
        .map(|threads| {
            threads
                .iter()
                .filter_map(|t| {
                    let raw = t["status"]["type"].as_str().unwrap_or("unknown");
                    let id = t["id"].as_str().filter(|id| !id.is_empty())?;
                    let spawn = &t["source"]["subAgent"]["thread_spawn"];
                    let parent = t["parentThreadId"]
                        .as_str()
                        .or_else(|| spawn["parent_thread_id"].as_str())
                        .filter(|id| !id.is_empty());
                    let is_subagent = parent.is_some() || t["source"].get("subAgent").is_some();
                    if raw == "notLoaded" && !is_subagent {
                        return None;
                    }
                    let waiting = t["status"]["activeFlags"].as_array().is_some_and(|a| {
                        a.iter()
                            .any(|f| f == "waitingOnApproval" || f == "waitingOnUserInput")
                    });
                    Some(AgentSession {
                        id: format!("codex:{id}"),
                        source: "codex".into(),
                        agent: "codex".into(),
                        title: t["agentNickname"]
                            .as_str()
                            .or_else(|| spawn["agent_nickname"].as_str())
                            .or(t["name"].as_str())
                            .or(t["preview"].as_str())
                            .unwrap_or("Codex session")
                            .chars()
                            .take(100)
                            .collect(),
                        status: if raw == "notLoaded" {
                            "unloaded".into()
                        } else if waiting {
                            "attention".into()
                        } else {
                            crate::agents::normalize(raw).into()
                        },
                        raw_status: raw.into(),
                        session_id: Some(id.into()),
                        target: id.into(),
                        updated_at: now(),
                        parent_session_id: parent.map(str::to_owned),
                        is_subagent,
                        can_focus: cfg!(target_os = "macos") && crate::focus::codex_url(id).is_ok(),
                        can_prompt: false,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}
pub async fn sessions() -> Result<Vec<AgentSession>, String> {
    let mut child = tokio::process::Command::new(crate::agents::binary("codex"))
        .args(["app-server", "proxy"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| e.to_string())?;
    let stream = ProxyStream {
        input: child.stdin.take().ok_or("Missing Codex proxy stdin")?,
        output: child.stdout.take().ok_or("Missing Codex proxy stdout")?,
    };
    let task = async {
        let (mut socket, _) = tokio_tungstenite::client_async("ws://localhost/", stream)
            .await
            .map_err(|e| format!("Codex WebSocket handshake: {e}"))?;
        snapshot(&mut socket).await
    };
    let result = tokio::time::timeout(std::time::Duration::from_secs(4), task)
        .await
        .map_err(|_| "Codex app-server connection timed out".to_string())?;
    let _ = child.kill().await;
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn malformed_registry_rejected_rpc_and_repeated_cursor_are_errors() {
        for results in [
            vec![json!({"data":null})],
            vec![json!({"data":[17]})],
            vec![
                json!({"data":[],"nextCursor":"same"}),
                json!({"data":[],"nextCursor":"same"}),
            ],
        ] {
            let (client, server) = tokio::io::duplex(8192);
            let server = tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(server).await.unwrap();
                ws.next().await.unwrap().unwrap();
                ws.send(Message::Text(
                    json!({"id":1,"result":{}}).to_string().into(),
                ))
                .await
                .unwrap();
                ws.next().await.unwrap().unwrap(); // initialized
                for result in results {
                    let request: Value = serde_json::from_str(
                        &ws.next().await.unwrap().unwrap().into_text().unwrap(),
                    )
                    .unwrap();
                    ws.send(Message::Text(
                        json!({"id":request["id"],"result":result})
                            .to_string()
                            .into(),
                    ))
                    .await
                    .unwrap();
                }
            });
            let (mut ws, _) = tokio_tungstenite::client_async("ws://localhost/", client)
                .await
                .unwrap();
            assert!(snapshot(&mut ws).await.is_err());
            server.await.unwrap();
        }
        for reply in [
            Some(json!({"id":1,"error":{"message":"fixture rejected"}})),
            None,
        ] {
            let (client, server) = tokio::io::duplex(8192);
            let server = tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(server).await.unwrap();
                ws.next().await.unwrap().unwrap();
                if let Some(reply) = reply {
                    ws.send(Message::Text(reply.to_string().into()))
                        .await
                        .unwrap();
                } else {
                    ws.close(None).await.unwrap();
                }
            });
            let (mut ws, _) = tokio_tungstenite::client_async("ws://localhost/", client)
                .await
                .unwrap();
            assert!(snapshot(&mut ws).await.is_err());
            server.await.unwrap();
        }
    }
    #[tokio::test]
    async fn actual_websocket_discovers_paginated_children_without_reading_turns() {
        let (client, server) = tokio::io::duplex(16384);
        let server = tokio::spawn(async move {
            let mut ws = tokio_tungstenite::accept_async(server).await.unwrap();
            let requests = [
                ("initialize", json!({})),
                ("initialized", Value::Null),
                (
                    "thread/loaded/list",
                    json!({"data":["parent"],"nextCursor":"next"}),
                ),
                (
                    "thread/loaded/list",
                    json!({"data":["child","parent"],"nextCursor":null}),
                ),
                (
                    "thread/read",
                    json!({"thread":{"id":"child","agentNickname":"Reviewer","parentThreadId":"parent","status":{"type":"active","activeFlags":["waitingOnApproval"]}}}),
                ),
                (
                    "thread/read",
                    json!({"thread":{"id":"parent","status":{"type":"idle"}}}),
                ),
                (
                    "thread/list",
                    json!({"data":[{"id":"old-child","parentThreadId":"parent","status":{"type":"notLoaded"}},{"id":"child","parentThreadId":"parent","status":{"type":"notLoaded"}}],"nextCursor":"desc-next"}),
                ),
                ("thread/list", json!({"data":[],"nextCursor":null})),
            ];
            for (i, (method, result)) in requests.into_iter().enumerate() {
                let text = ws.next().await.unwrap().unwrap().into_text().unwrap();
                let request: Value = serde_json::from_str(&text).unwrap();
                assert_eq!(request["method"], method);
                if method == "initialize" {
                    assert_eq!(request["params"]["capabilities"]["experimentalApi"], true);
                }
                if method == "thread/list" {
                    assert_eq!(request["params"]["ancestorThreadId"], "parent");
                    assert!(request["params"]["sourceKinds"]
                        .as_array()
                        .unwrap()
                        .contains(&json!("subAgentThreadSpawn")));
                }
                if i == 7 {
                    assert_eq!(request["params"]["cursor"], "desc-next");
                }
                if method == "initialized" {
                    continue;
                }
                if method == "thread/read" {
                    assert_eq!(request["params"]["includeTurns"], false);
                }
                if i == 3 {
                    assert_eq!(request["params"]["cursor"], "next");
                }
                if i == 4 {
                    assert_eq!(request["params"]["threadId"], "child");
                }
                ws.send(Message::Text(
                    json!({"id":request["id"],"result":result})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
            }
        });
        let (mut ws, _) = tokio_tungstenite::client_async("ws://localhost/", client)
            .await
            .unwrap();
        let agents = snapshot(&mut ws).await.unwrap();
        assert_eq!(agents.len(), 3);
        assert_eq!(agents[2].status, "unloaded");
        assert_eq!(agents[0].status, "attention");
        assert_eq!(agents[0].target, "child");
        assert_eq!(agents[0].title, "Reviewer");
        assert!(agents[0].is_subagent);
        assert_eq!(agents[0].parent_session_id.as_deref(), Some("parent"));
        assert_eq!(agents[1].status, "ready");
        server.await.unwrap();
    }
    #[test]
    fn thread_spawn_lineage_and_same_titles_keep_distinct_targets() {
        let agents = parse_threads(&json!({"data":[
            {"id":"one","name":"Same","status":{"type":"active"},"source":{"subAgent":{"thread_spawn":{"parent_thread_id":"parent","agent_nickname":"Worker"}}}},
            {"id":"two","name":"Same","status":{"type":"idle"}}
        ]}));
        assert_eq!(agents.len(), 2);
        assert_eq!(agents[0].parent_session_id.as_deref(), Some("parent"));
        assert_eq!(agents[0].target, "one");
        assert_eq!(agents[1].target, "two");
    }
    #[test]
    fn all_runtime_states() {
        for (raw, expected) in [
            ("active", "working"),
            ("idle", "ready"),
            ("systemError", "unknown"),
            ("newState", "unknown"),
        ] {
            let a = parse_threads(&json!({"data":[{"id":"t","status":{"type":raw}}]}));
            assert_eq!(a[0].status, expected);
        }
        assert!(
            parse_threads(&json!({"data":[{"id":"t","status":{"type":"notLoaded"}}]})).is_empty()
        );
    }
}
