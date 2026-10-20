//! Test-only loopback fixture: real updater HTTP and signatures, no installer.
use super::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    task::JoinHandle,
};

pub struct Feed {
    pub url: String,
    pub public_key: String,
    task: JoinHandle<()>,
}
impl Drop for Feed {
    fn drop(&mut self) {
        self.task.abort();
    }
}
#[derive(serde::Deserialize)]
pub struct Fixture {
    pub public_key: String,
    pub signature: String,
    pub seed: String,
    pub repeat: usize,
}
pub fn fixture() -> Fixture {
    serde_json::from_str(include_str!("../../../tests/fixtures/updater.json"))
        .expect("checked-in updater fixture")
}
impl Feed {
    pub async fn start() -> Result<Self, String> {
        Self::serve("available").await
    }
    pub async fn serve(scenario: &str) -> Result<Self, String> {
        let fixture = fixture();
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|e| e.to_string())?;
        let origin = format!(
            "http://{}",
            listener.local_addr().map_err(|e| e.to_string())?
        );
        let url = format!("{origin}/latest.json");
        let public_key = fixture.public_key.clone();
        let scenario = scenario.to_owned();
        let task = tokio::spawn(async move {
            // Serial requests keep the fixture bounded and are sufficient for one updater.
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut request = [0u8; 4096];
                let Ok(Ok(n)) =
                    tokio::time::timeout(Duration::from_secs(2), socket.read(&mut request)).await
                else {
                    continue;
                };
                if n == 0 {
                    continue;
                }
                let manifest_request = request[..n].starts_with(b"GET /latest.json ");
                if scenario == "timeout" {
                    tokio::time::sleep(Duration::from_secs(60)).await;
                    continue;
                }
                let manifest = serde_json::json!({
                    "version": if scenario == "current" { "0.0.0" } else { "9999.0.0" },
                    "notes": "Local verification sample. Downloads a signed test payload; does not install or restart the application.",
                    "platforms": {
                        "test-target": { "url": format!("{origin}/payload"), "signature": fixture.signature }
                    }
                });
                let mut body = if manifest_request {
                    if scenario == "malformed" {
                        b"invalid json".to_vec()
                    } else if scenario == "platform" {
                        serde_json::to_vec(
                            &serde_json::json!({"version":"9999.0.0","platforms":{}}),
                        )
                        .unwrap()
                    } else {
                        serde_json::to_vec(&manifest).unwrap()
                    }
                } else {
                    fixture.seed.repeat(fixture.repeat).into_bytes()
                };
                if scenario == "corrupt" && !manifest_request {
                    body[0] ^= 1;
                }
                let status = if scenario == "http" {
                    "503 Service Unavailable"
                } else {
                    "200 OK"
                };
                let size = body.len()
                    + if scenario == "interrupted" && !manifest_request {
                        100
                    } else {
                        0
                    };
                let header = format!("HTTP/1.1 {status}\r\nContent-Length: {size}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n");
                if socket.write_all(header.as_bytes()).await.is_err() {
                    continue;
                }
                for chunk in body.chunks(8192) {
                    if socket.write_all(chunk).await.is_err() {
                        break;
                    }
                    if !manifest_request {
                        tokio::time::sleep(Duration::from_millis(35)).await;
                    }
                }
            }
        });
        Ok(Self {
            url,
            public_key,
            task,
        })
    }
}
pub fn builder<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    feed: &Feed,
) -> Result<tauri_plugin_updater::UpdaterBuilder, String> {
    app.updater_builder()
        .pubkey(&feed.public_key)
        .target("test-target")
        .timeout(Duration::from_secs(30))
        .no_proxy()
        .endpoints(vec![feed
            .url
            .parse()
            .map_err(|_| "Invalid local preview URL")?])
        .map_err(update_error)
}
