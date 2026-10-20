use crate::model::*;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub fn delta(kind: &str, v: &Value) -> String {
    match kind {
        "openai" => {
            if v["type"] == "response.output_text.delta" {
                v["delta"].as_str().unwrap_or_default().into()
            } else {
                String::new()
            }
        }
        "anthropic" => v["delta"]["text"].as_str().unwrap_or_default().into(),
        "gemini" => v["candidates"]
            .as_array()
            .and_then(|a| a.first())
            .and_then(|v| v["content"]["parts"].as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|v| v["text"].as_str())
                    .collect::<String>()
            })
            .unwrap_or_default(),
        _ => v["choices"][0]["delta"]["content"]
            .as_str()
            .unwrap_or_default()
            .into(),
    }
}
const LIMIT: usize = 2_000_000;

fn validate_provider(provider: &Provider) -> Result<reqwest::Url, String> {
    if provider.model.trim().is_empty() {
        return Err("Choose a provider model in Settings".into());
    }
    if !["openai", "anthropic", "gemini", "compatible"].contains(&provider.kind.as_str()) {
        return Err("Unsupported AI provider kind".into());
    }
    let url = reqwest::Url::parse(provider.endpoint.trim_end_matches('/'))
        .map_err(|_| "Invalid provider endpoint")?;
    if !["http", "https"].contains(&url.scheme()) || url.host_str().is_none() {
        return Err("Provider must use HTTP or HTTPS".into());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Provider endpoint must not contain credentials, query parameters or fragments".into(),
        );
    }
    Ok(url)
}

/// SSE framing operates on bytes so fragmented UTF-8 is decoded only after a full
/// line arrives. Multiple data fields form one event, as required by SSE.
#[derive(Default)]
struct Events {
    pending: Vec<u8>,
    data: Vec<String>,
    data_bytes: usize,
}
impl Events {
    fn push(&mut self, bytes: &[u8]) -> Result<Vec<String>, String> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(pos) = self.pending.iter().position(|b| *b == b'\n') {
            if pos > LIMIT {
                return Err("Provider event exceeds 2 MB".into());
            }
            let line: Vec<_> = self.pending.drain(..=pos).collect();
            let line = std::str::from_utf8(&line[..line.len() - 1])
                .map_err(|_| "Provider sent invalid UTF-8")?
                .trim_end_matches('\r');
            if line.is_empty() {
                if !self.data.is_empty() {
                    events.push(self.data.join("\n"));
                }
                self.data.clear();
                self.data_bytes = 0;
            } else if let Some(value) = line.strip_prefix("data:") {
                let value = value.strip_prefix(' ').unwrap_or(value);
                self.data_bytes += value.len() + 1;
                if self.data_bytes > LIMIT {
                    return Err("Provider event exceeds 2 MB".into());
                }
                self.data.push(value.to_owned());
            }
        }
        if self.pending.len() > LIMIT {
            return Err("Provider event exceeds 2 MB".into());
        }
        Ok(events)
    }
}

fn event_text(kind: &str, data: &str) -> Result<(String, bool), String> {
    if data == "[DONE]" {
        return if kind == "compatible" {
            Ok((String::new(), true))
        } else {
            Err("Provider ended without a completion event".into())
        };
    }
    let value: Value = serde_json::from_str(data).map_err(|_| "Invalid provider JSON event")?;
    if !value.is_object() {
        return Err("Invalid provider event shape".into());
    }
    if value.get("error").is_some_and(|e| !e.is_null())
        || matches!(
            value["type"].as_str(),
            Some("error" | "response.failed" | "response.incomplete")
        )
    {
        return Err("Provider reported an error or incomplete response".into());
    }
    let complete = match kind {
        "openai" => value["type"] == "response.completed",
        "anthropic" => {
            if let Some(reason) = value["delta"]["stop_reason"].as_str() {
                if !["end_turn", "stop_sequence"].contains(&reason) {
                    return Err("Provider response stopped before text completion".into());
                }
            }
            value["type"] == "message_stop"
        }
        "gemini" => {
            if value["promptFeedback"]["blockReason"].is_string() {
                return Err("Provider blocked the prompt".into());
            }
            match value["candidates"][0]["finishReason"].as_str() {
                None => false,
                Some("STOP") => true,
                Some(_) => return Err("Provider response stopped before text completion".into()),
            }
        }
        _ => match value["choices"][0]["finish_reason"].as_str() {
            None => false,
            Some("stop") => true,
            Some(_) => return Err("Provider response stopped before text completion".into()),
        },
    };
    Ok((delta(kind, &value), complete))
}

/// Production HTTP transport with injectable credentials and observer; no Tauri,
/// keychain, hardware or real provider is needed to exercise this path in CI.
async fn generate_http(
    provider: &Provider,
    prompt: &str,
    key: &str,
    cancel: &AtomicBool,
    timeout: std::time::Duration,
    mut output: impl FnMut(&str),
) -> Result<String, String> {
    let mut url = validate_provider(provider)?;
    if key.is_empty() && provider.kind != "compatible" {
        return Err("Add an API key for this provider".into());
    }
    let operation = async {
        let client = reqwest::Client::builder()
            // Custom API-key headers must never follow redirects to another host.
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| "Could not initialize provider transport")?;
        let path = url.path().trim_end_matches('/').to_owned();
        let suffix = match provider.kind.as_str() {
            "openai" => "responses".to_owned(),
            "anthropic" => "messages".to_owned(),
            "gemini" => {
                // A model is one URL segment, not a new path/query supplied by the model field.
                if !provider
                    .model
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
                {
                    return Err("Invalid Gemini model identifier".into());
                }
                format!("models/{}:streamGenerateContent", provider.model)
            }
            _ => "chat/completions".to_owned(),
        };
        url.set_path(&format!("{path}/{suffix}"));
        if provider.kind == "gemini" {
            url.set_query(Some("alt=sse"));
        }
        let request = client.post(url);
        let request = match provider.kind.as_str() {
            "openai" => request.bearer_auth(key).json(&json!({"model":provider.model,"input":prompt,"stream":true,"store":false})),
            "anthropic" => request.header("x-api-key",key).header("anthropic-version","2023-06-01").json(&json!({"model":provider.model,"messages":[{"role":"user","content":prompt}],"max_tokens":4096,"stream":true})),
            "gemini" => request.header("x-goog-api-key",key).json(&json!({"contents":[{"parts":[{"text":prompt}]}]})),
            _ => {
                let request = if key.is_empty() { request } else { request.bearer_auth(key) };
                request.json(&json!({"model":provider.model,"messages":[{"role":"user","content":prompt}],"stream":true}))
            }
        };
        // Do not expose reqwest error strings: URLs can contain private endpoint information.
        let response = request
            .send()
            .await
            .map_err(|_| "Provider connection failed")?;
        if !response.status().is_success() {
            return Err(format!(
                "Provider returned HTTP {}; verify model, credentials and quota",
                response.status()
            ));
        }
        if !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| {
                h.split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("text/event-stream"))
            })
        {
            return Err("Provider did not return an event stream".into());
        }
        let mut stream = response.bytes_stream();
        let mut events = Events::default();
        let mut result = String::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "Provider stream interrupted")?;
            for data in events.push(&chunk)? {
                let (text, complete) = event_text(&provider.kind, &data)?;
                result.push_str(&text);
                if result.len() > LIMIT {
                    return Err("Output exceeds 2 MB".into());
                }
                if !text.is_empty() {
                    output(&result);
                }
                if complete {
                    return if result.is_empty() {
                        Err("Provider returned no text".into())
                    } else {
                        Ok(result)
                    };
                }
            }
        }
        Err("Provider stream ended before completion".into())
    };
    let cancelled = async {
        loop {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    };
    tokio::select! {
        biased;
        _ = cancelled => Err("Cancelled".into()),
        result = tokio::time::timeout(timeout, operation) => result.unwrap_or_else(|_| Err("Provider request timed out".into())),
    }
}

/// Imported configs may reuse an ID while changing its endpoint. Credentials
/// belong to the endpoint identity, never the portable provider ID alone.
pub fn credential_account(provider: &Provider) -> String {
    use sha2::{Digest, Sha256};
    use std::fmt::Write;
    let endpoint = reqwest::Url::parse(provider.endpoint.trim_end_matches('/'))
        .map(|url| url.to_string())
        .unwrap_or_else(|_| provider.endpoint.clone());
    let mut identity = Sha256::new();
    for part in [&provider.id, &provider.kind, &endpoint] {
        identity.update((part.len() as u64).to_be_bytes());
        identity.update(part.as_bytes());
    }
    let mut account = String::from("provider-endpoint:");
    for byte in identity.finalize() {
        write!(&mut account, "{byte:02x}").expect("writing to a String cannot fail");
    }
    account
}

pub async fn generate(
    provider: &Provider,
    prompt: &str,
    cancel: &Arc<AtomicBool>,
) -> Result<String, String> {
    validate_provider(provider)?;
    let account = credential_account(provider);
    let key = tokio::task::spawn_blocking(move || {
        match keyring::Entry::new("OpenStreamDeck", &account)
            .map_err(|_| "Could not access provider credentials")?
            .get_password()
        {
            Ok(key) => Ok(key),
            Err(keyring::Error::NoEntry) => Ok(String::new()),
            Err(_) => Err(
                "Could not read provider credentials; unlock the operating system credential store",
            ),
        }
    })
    .await
    .map_err(|_| "Credential lookup failed")??;
    generate_http(
        provider,
        prompt,
        &key,
        cancel,
        std::time::Duration::from_secs(180),
        |_| {},
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn credential_account_preserves_persisted_identity_across_dependency_upgrades() {
        // Existing OS entries use this exact account name. No credential store
        // is opened; this fixture contains only a synthetic provider identity.
        for endpoint in [
            "https://api.openai.com/v1",
            "https://API.OPENAI.COM:443/v1/",
        ] {
            assert_eq!(
                credential_account(&provider("openai", endpoint.into())),
                "provider-endpoint:39d93f76dbe9ffd1a92be1c6b2770e45de64ada68f2843776b53f73c66f05d74"
            );
        }
    }

    #[test]
    fn imported_endpoint_cannot_reuse_an_existing_credential() {
        let original = provider("openai", "https://api.openai.com/v1".into());
        let original_account = credential_account(&original);
        let mut imported = original.clone();
        imported.endpoint = "https://other.example/v1".into();
        assert_ne!(credential_account(&imported), original_account);
        imported = original.clone();
        imported.kind = "compatible".into();
        assert_ne!(credential_account(&imported), original_account);
        imported = original.clone();
        imported.id = "other".into();
        assert_ne!(credential_account(&imported), original_account);
        imported = original.clone();
        imported.endpoint.push('/');
        assert_eq!(credential_account(&imported), original_account);
        assert!(!original_account.contains("api.openai.com"));
    }

    fn provider(kind: &str, endpoint: String) -> Provider {
        Provider {
            id: "test".into(),
            name: "Test".into(),
            kind: kind.into(),
            endpoint,
            model: "test-model".into(),
        }
    }
    // A real HTTP listener captures requests, then writes fragmented responses.
    // Each test gets a fresh ephemeral loopback port; no credentials or hardware.
    async fn server(
        status: u16,
        body: String,
        stall: u8,
    ) -> (
        String,
        tokio::task::JoinHandle<String>,
        tokio::sync::oneshot::Receiver<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}/v1", listener.local_addr().unwrap());
        let (ready_tx, ready) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut bytes = [0u8; 1024];
            loop {
                let n = socket.read(&mut bytes).await.unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&bytes[..n]);
                if let Some(pos) = request.windows(4).position(|w| w == b"\r\n\r\n") {
                    let headers = String::from_utf8_lossy(&request[..pos]).to_lowercase();
                    let size: usize = headers
                        .lines()
                        .find_map(|line| line.strip_prefix("content-length: "))
                        .unwrap()
                        .parse()
                        .unwrap();
                    if request.len() >= pos + 4 + size {
                        break;
                    }
                }
            }
            let _ = ready_tx.send(());
            if stall == 1 {
                // Waiting for EOF lets cancellation/timeout own termination.
                let _ = socket.read_to_end(&mut Vec::new()).await;
            } else {
                let redirect = if status == 302 {
                    "Location: http://127.0.0.1:1/private-redirect\r\n"
                } else {
                    ""
                };
                let headers = format!("HTTP/1.1 {status} Test\r\n{redirect}Content-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len() + usize::from(stall == 2));
                let _ = socket.write_all(headers.as_bytes()).await;
                for chunk in body.as_bytes().chunks(3) {
                    if socket.write_all(chunk).await.is_err() {
                        break;
                    }
                    tokio::task::yield_now().await;
                }
                if stall == 2 {
                    let _ = socket.read_to_end(&mut Vec::new()).await;
                }
            }
            String::from_utf8(request).unwrap()
        });
        (endpoint, task, ready)
    }
    fn stream(kind: &str) -> String {
        let (delta, end) = match kind {
            "openai" => (
                json!({"type":"response.output_text.delta","delta":"héllo"}),
                json!({"type":"response.completed"}),
            ),
            "anthropic" => (
                json!({"type":"content_block_delta","delta":{"text":"héllo"}}),
                json!({"type":"message_stop"}),
            ),
            "gemini" => (
                json!({"candidates":[{"content":{"parts":[{"text":"héllo"}]}}]}),
                json!({"candidates":[{"finishReason":"STOP"}]}),
            ),
            _ => (
                json!({"choices":[{"delta":{"content":"héllo"}}]}),
                json!({"choices":[{"finish_reason":"stop"}]}),
            ),
        };
        format!(": heartbeat\r\ndata: {delta}\r\n\r\ndata: {end}\r\n\r\n")
    }
    #[tokio::test]
    async fn all_provider_http_contracts() {
        for kind in ["openai", "anthropic", "gemini", "compatible"] {
            let (endpoint, request, _) = server(200, stream(kind), 0).await;
            let mut observed = Vec::new();
            let result = generate_http(
                &provider(kind, endpoint),
                "explicit input",
                "fixture-key",
                &AtomicBool::new(false),
                Duration::from_secs(3),
                |text| observed.push(text.to_owned()),
            )
            .await
            .unwrap();
            assert_eq!(result, "héllo");
            assert_eq!(observed.last().unwrap(), "héllo");
            let request = request.await.unwrap();
            let (headers, body) = request.split_once("\r\n\r\n").unwrap();
            let body: Value = serde_json::from_str(body).unwrap();
            assert!(!request.contains("?key="));
            match kind {
                "openai" => {
                    assert!(headers.starts_with("POST /v1/responses "));
                    assert!(headers.contains("authorization: Bearer fixture-key"));
                    assert_eq!(body["store"], false);
                    assert_eq!(body["input"], "explicit input");
                }
                "anthropic" => {
                    assert!(headers.starts_with("POST /v1/messages "));
                    assert!(headers.contains("x-api-key: fixture-key"));
                    assert!(headers.contains("anthropic-version: 2023-06-01"));
                    assert_eq!(body["max_tokens"], 4096);
                    assert_eq!(body["messages"][0]["content"], "explicit input");
                }
                "gemini" => {
                    assert!(headers
                        .starts_with("POST /v1/models/test-model:streamGenerateContent?alt=sse "));
                    assert!(headers.contains("x-goog-api-key: fixture-key"));
                    assert_eq!(body["contents"][0]["parts"][0]["text"], "explicit input");
                }
                _ => {
                    assert!(headers.starts_with("POST /v1/chat/completions "));
                    assert!(headers.contains("authorization: Bearer fixture-key"));
                    assert_eq!(body["messages"][0]["content"], "explicit input");
                }
            }
            if kind != "gemini" {
                assert_eq!(body["model"], "test-model");
                assert_eq!(body["stream"], true);
            }
        }
    }
    #[tokio::test]
    async fn all_providers_errors_truncation_and_empty_completion() {
        for kind in ["openai", "anthropic", "gemini", "compatible"] {
            for (status, body, expected) in [
                (401, "private secret".to_owned(), "HTTP 401"),
                (302, String::new(), "HTTP 302"),
                (
                    200,
                    stream(kind).split("\r\n\r\n").nth(1).unwrap().to_owned() + "\r\n\r\n",
                    "no text",
                ),
                (
                    200,
                    "data: {\"error\":\"private secret\"}\n\n".into(),
                    "error",
                ),
                (200, "data: invalid private secret\n\n".into(), "Invalid"),
                (
                    200,
                    stream(kind).split("\r\n\r\n").next().unwrap().to_owned() + "\r\n\r\n",
                    "before completion",
                ),
            ] {
                let (endpoint, request, _) = server(status, body, 0).await;
                let error = generate_http(
                    &provider(kind, endpoint),
                    "input",
                    "fixture-key",
                    &AtomicBool::new(false),
                    Duration::from_secs(3),
                    |_| {},
                )
                .await
                .unwrap_err();
                assert!(error.contains(expected), "{kind}: {error}");
                assert!(!error.contains("private secret"));
                request.await.unwrap();
            }
        }
    }
    #[tokio::test]
    async fn cancellation_and_timeout_cover_headers_and_body_for_every_provider() {
        for kind in ["openai", "anthropic", "gemini", "compatible"] {
            for (cancel_request, phase) in [(true, 1), (false, 1), (true, 2), (false, 2)] {
                let body = stream(kind).split("\r\n\r\n").next().unwrap().to_owned() + "\r\n\r\n";
                let (endpoint, request, ready) = server(200, body, phase).await;
                let cancel = Arc::new(AtomicBool::new(false));
                let task_cancel = cancel.clone();
                let trigger = tokio::spawn(async move {
                    let _ = ready.await;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    if cancel_request {
                        task_cancel.store(true, Ordering::Relaxed);
                    }
                });
                let error = generate_http(
                    &provider(kind, endpoint),
                    "input",
                    "fixture-key",
                    &cancel,
                    Duration::from_millis(500),
                    |_| {},
                )
                .await
                .unwrap_err();
                assert_eq!(
                    error,
                    if cancel_request {
                        "Cancelled"
                    } else {
                        "Provider request timed out"
                    }
                );
                trigger.await.unwrap();
                tokio::time::timeout(Duration::from_secs(2), request)
                    .await
                    .unwrap()
                    .unwrap();
            }
        }
    }
    #[tokio::test]
    async fn pre_cancel_missing_key_and_keyless_local_provider() {
        let endpoint = "http://127.0.0.1:1/v1".to_owned();
        for kind in ["openai", "anthropic", "gemini", "compatible"] {
            let error = generate_http(
                &provider(kind, endpoint.clone()),
                "input",
                "fixture-key",
                &AtomicBool::new(true),
                Duration::from_secs(1),
                |_| {},
            )
            .await
            .unwrap_err();
            assert_eq!(error, "Cancelled");
            if kind != "compatible" {
                let error = generate_http(
                    &provider(kind, endpoint.clone()),
                    "input",
                    "",
                    &AtomicBool::new(false),
                    Duration::from_secs(1),
                    |_| {},
                )
                .await
                .unwrap_err();
                assert!(error.contains("API key"));
            }
        }
        let (endpoint, request, _) = server(200, stream("compatible"), 0).await;
        assert_eq!(
            generate_http(
                &provider("compatible", endpoint),
                "input",
                "",
                &AtomicBool::new(false),
                Duration::from_secs(3),
                |_| {}
            )
            .await
            .unwrap(),
            "héllo"
        );
        assert!(!request.await.unwrap().contains("authorization:"));
    }
    #[test]
    fn framing_multiline_utf8_and_limits() {
        let mut events = Events::default();
        let mut output = Vec::new();
        for byte in
            b": ping\r\nevent: response\r\ndata: {\r\ndata: \"text\":\"h\xc3\xa9llo\"}\r\n\r\n"
        {
            output.extend(events.push(&[*byte]).unwrap());
        }
        assert_eq!(output, vec!["{\n\"text\":\"héllo\"}"]);
        assert!(Events::default().push(&[0xff, b'\n']).is_err());
        assert!(Events::default().push(&vec![b'a'; LIMIT + 1]).is_err());
        assert!(Events::default()
            .push(format!("data: {}\n", "a".repeat(LIMIT)).as_bytes())
            .is_err());
    }
    #[test]
    fn malformed_configuration_and_non_success_terminal_events() {
        for endpoint in [
            "invalid",
            "file:///tmp/key",
            "http://user:secret@localhost",
            "http://localhost?key=secret",
        ] {
            assert!(validate_provider(&provider("openai", endpoint.into())).is_err());
        }
        assert!(validate_provider(&provider("unknown", "http://localhost".into())).is_err());
        let mut no_model = provider("openai", "http://localhost".into());
        no_model.model.clear();
        assert!(validate_provider(&no_model).is_err());
        for (kind, event) in [
            ("openai", json!({"type":"response.incomplete"})),
            ("anthropic", json!({"delta":{"stop_reason":"max_tokens"}})),
            ("gemini", json!({"candidates":[{"finishReason":"SAFETY"}]})),
            (
                "compatible",
                json!({"choices":[{"finish_reason":"length"}]}),
            ),
        ] {
            assert!(event_text(kind, &event.to_string()).is_err());
        }
        assert!(event_text("openai", "[DONE]").is_err());
        assert!(event_text("compatible", "[DONE]").unwrap().1);
    }
}
