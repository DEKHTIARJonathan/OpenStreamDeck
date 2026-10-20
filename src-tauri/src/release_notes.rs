//! Release-note links open in the OS browser, never in the privileged app webview.

fn open_link(url: &str, open: impl FnOnce(&str) -> Result<(), String>) -> Result<(), String> {
    if url.chars().any(char::is_control) {
        return Err("Invalid release-note link.".into());
    }
    let parsed = reqwest::Url::parse(url).map_err(|_| "Invalid release-note link.")?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err("Release-note links must use HTTP or HTTPS.".into());
    }
    open(parsed.as_str()).map_err(|error| format!("Could not open release-note link: {error}"))
}

#[tauri::command]
pub async fn open_release_link(url: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        open_link(&url, |target| {
            open::that(target).map_err(|error| error.to_string())
        })
    })
    .await
    .map_err(|error| format!("Could not open release-note link: {error}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_links_use_the_native_boundary_and_report_os_failures() {
        for url in [
            "https://example.com/notes?version=1#changes",
            "http://localhost:8080/notes",
        ] {
            let mut opened = None;
            open_link(url, |target| {
                opened = Some(target.to_owned());
                Ok(())
            })
            .unwrap();
            assert_eq!(opened.as_deref(), Some(url));
            assert_eq!(
                open_link(url, |_| Err("browser unavailable".into())).unwrap_err(),
                "Could not open release-note link: browser unavailable"
            );
        }
    }

    #[test]
    fn non_web_links_never_reach_the_os() {
        for url in [
            "javascript:alert(1)",
            "file:///tmp/payload",
            "data:text/html,hello",
            "mailto:user@example.com",
            "docs/guide.md",
            "",
            "https://",
            "https://user:password@example.com/",
            "https://exa\nmple.com/",
        ] {
            assert!(open_link(url, |_| panic!("unsafe link reached OS: {url}")).is_err());
        }
    }
}
