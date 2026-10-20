use base64::Engine;
use serde::Serialize;
use std::path::{Path, PathBuf};
#[derive(Clone, Serialize)]
pub struct Application {
    pub name: String,
    pub path: String,
}
#[cfg(any(target_os = "macos", test))]
fn collect(directory: &Path, depth: usize, apps: &mut Vec<Application>) {
    if depth > 2 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e == "app") {
            apps.push(Application {
                name: p.file_stem().unwrap_or_default().to_string_lossy().into(),
                path: p.display().to_string(),
            });
        } else if p.is_dir() {
            collect(&p, depth + 1, apps)
        }
    }
}
pub fn list() -> Vec<Application> {
    let mut apps = vec![];
    #[cfg(target_os = "macos")]
    {
        for dir in [
            PathBuf::from("/Applications"),
            PathBuf::from("/System/Applications"),
        ] {
            collect(&dir, 0, &mut apps)
        }
        if let Some(home) = std::env::var_os("HOME") {
            collect(&PathBuf::from(home).join("Applications"), 0, &mut apps)
        }
    }
    #[cfg(target_os = "linux")]
    {
        for dir in [
            PathBuf::from("/usr/share/applications"),
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".local/share/applications"),
        ] {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().is_some_and(|e| e == "desktop") {
                        if let Ok(text) = std::fs::read_to_string(&p) {
                            if text
                                .lines()
                                .any(|l| l == "NoDisplay=true" || l == "Hidden=true")
                            {
                                continue;
                            }
                            let name = text
                                .lines()
                                .find_map(|l| l.strip_prefix("Name="))
                                .unwrap_or("Application");
                            apps.push(Application {
                                name: name.into(),
                                path: p.display().to_string(),
                            })
                        }
                    }
                }
            }
        }
    }
    #[cfg(windows)]
    for variable in ["APPDATA", "PROGRAMDATA"] {
        if let Some(root) = std::env::var_os(variable) {
            collect_shortcuts(
                &PathBuf::from(root).join("Microsoft/Windows/Start Menu/Programs"),
                0,
                &mut apps,
            );
        }
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.path == b.path);
    apps
}
/// Association selection is independent of Open App binding/artwork mutation.
/// Identity failure rejects the entire selection; unavailable artwork is optional.
pub async fn association(path: &str) -> Result<crate::model::ApplicationAssociation, String> {
    let target = path.to_owned();
    association_with(
        async move {
            tokio::task::spawn_blocking(move || crate::foreground::application_identity(&target))
                .await
                .map_err(|e| format!("Application identity lookup failed: {e}"))?
        },
        || {
            list()
                .into_iter()
                .find(|app| app.path == path)
                .map(|app| app.name)
                .unwrap_or_else(|| {
                    Path::new(path)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned()
                })
        },
        || icon(path),
    )
    .await
}
async fn association_with<I, N, F, O>(
    identity: I,
    name: N,
    icon: F,
) -> Result<crate::model::ApplicationAssociation, String>
where
    I: std::future::Future<Output = Result<crate::model::ApplicationIdentity, String>>,
    N: FnOnce() -> String,
    F: FnOnce() -> O,
    O: std::future::Future<Output = Result<String, String>>,
{
    let identity = identity.await?;
    identity.validate()?;
    Ok(crate::model::ApplicationAssociation {
        name: name(),
        identities: vec![identity],
        image: icon().await.ok(),
    })
}

pub async fn icon(path: &str) -> Result<String, String> {
    if !Path::new(path).exists() {
        return Err("Application no longer exists".into());
    }
    #[cfg(target_os = "macos")]
    let bytes = {
        // AppKit icon lookup works for .app bundles, including Assets.car-based icons.
        // The path is a separate argv item, never interpolated into executable code.
        let script = r#"ObjC.import('AppKit'); function run(argv) { const icon=$.NSWorkspace.sharedWorkspace.iconForFile($(argv[0])); const rep=$.NSBitmapImageRep.imageRepWithData(icon.TIFFRepresentation); const data=rep.representationUsingTypeProperties($.NSBitmapImageFileTypePNG,$({})); return data.base64EncodedStringWithOptions(0).js; }"#;
        let encoded = crate::agents::output(
            "/usr/bin/osascript",
            &["-l", "JavaScript", "-e", script, path],
        )
        .await?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|e| e.to_string())?
    };
    #[cfg(windows)]
    let bytes = {
        let script = r#"$ErrorActionPreference='Stop'; Add-Type -AssemblyName System.Drawing; $p=$env:OPENSTREAMDECK_ICON_PATH; if([IO.Path]::GetExtension($p) -eq '.lnk'){$p=(New-Object -ComObject WScript.Shell).CreateShortcut($p).TargetPath}; $i=[System.Drawing.Icon]::ExtractAssociatedIcon($p); if($null -eq $i){throw 'Application has no icon'}; $s=New-Object System.IO.MemoryStream; $i.ToBitmap().Save($s,[System.Drawing.Imaging.ImageFormat]::Png); [Convert]::ToBase64String($s.ToArray())"#;
        // PowerShell -Command reparses trailing arguments as code. Pass the path
        // through a private child environment variable instead.
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(4),
            tokio::process::Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", script])
                .env("OPENSTREAMDECK_ICON_PATH", path)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .map_err(|_| "Icon extraction timed out")?
        .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("Application icon extraction failed".into());
        }
        let encoded = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
        base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .map_err(|e| e.to_string())?
    };
    #[cfg(target_os = "linux")]
    let bytes = {
        let desktop = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let icon = desktop
            .lines()
            .find_map(|l| l.strip_prefix("Icon="))
            .ok_or("Desktop entry has no icon")?;
        let mut candidates = vec![PathBuf::from(icon)];
        for size in ["256x256", "128x128", "64x64", "48x48"] {
            for theme in ["hicolor", "Adwaita"] {
                candidates.push(PathBuf::from(format!(
                    "/usr/share/icons/{theme}/{size}/apps/{icon}.png"
                )))
            }
        }
        candidates.push(PathBuf::from(format!("/usr/share/pixmaps/{icon}.png")));
        std::fs::read(
            candidates
                .into_iter()
                .find(|p| p.is_file())
                .ok_or("Application icon was not found in the installed icon themes")?,
        )
        .map_err(|e| e.to_string())?
    };
    let image = image::load_from_memory(&bytes)
        .map_err(|e| e.to_string())?
        .resize(256, 256, image::imageops::FilterType::Lanczos3);
    let mut output = std::io::Cursor::new(Vec::new());
    image
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(output.into_inner())
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovers_bundles_without_descending_into_them() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("Sample.app/Contents/Nested.app")).unwrap();
        std::fs::create_dir_all(dir.path().join("Utilities/Second.app")).unwrap();
        let mut apps = vec![];
        collect(dir.path(), 0, &mut apps);
        assert_eq!(apps.len(), 2);
        assert!(apps.iter().any(|a| a.name == "Sample"));
    }
    #[tokio::test]
    async fn missing_app_is_an_explicit_error() {
        assert!(icon("/definitely-not-an-openstreamdeck-app")
            .await
            .unwrap_err()
            .contains("no longer exists"));
    }
}

/// App bundles use LaunchServices; .desktop files must be launched, not opened
/// in whichever text editor owns the file association.
pub fn launch_command(target: &str, platform: &str) -> Option<(&'static str, Vec<String>)> {
    match platform {
        "macos" if !target.contains('/') && !target.contains(':') => {
            Some(("/usr/bin/open", vec!["-a".into(), target.into()]))
        }
        "linux"
            if Path::new(target)
                .extension()
                .is_some_and(|e| e == "desktop") =>
        {
            Some(("gio", vec!["launch".into(), target.into()]))
        }
        _ => None,
    }
}
#[cfg(any(windows, test))]
fn collect_shortcuts(directory: &Path, depth: usize, apps: &mut Vec<Application>) {
    if depth > 8 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("lnk")) {
            apps.push(Application {
                name: p.file_stem().unwrap_or_default().to_string_lossy().into(),
                path: p.display().to_string(),
            });
        } else if p.is_dir() {
            collect_shortcuts(&p, depth + 1, apps);
        }
    }
}
#[cfg(test)]
mod launch_tests {
    use super::*;
    #[test]
    fn desktop_entries_launch_and_paths_remain_single_arguments() {
        let target = "/tmp/application with $special.desktop";
        assert_eq!(
            launch_command(target, "linux"),
            Some(("gio", vec!["launch".into(), target.into()]))
        );
        assert_eq!(
            launch_command("Terminal", "macos"),
            Some(("/usr/bin/open", vec!["-a".into(), "Terminal".into()]))
        );
        assert!(launch_command("https://example.com", "macos").is_none());
        assert!(launch_command("C:\\App.exe", "windows").is_none());
    }
    #[test]
    fn windows_start_menu_shortcuts_are_discovered_without_executing_them() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("Folder")).unwrap();
        std::fs::write(dir.path().join("Folder/Example.LNK"), "fixture").unwrap();
        std::fs::write(dir.path().join("Ignored.txt"), "fixture").unwrap();
        let mut apps = vec![];
        collect_shortcuts(dir.path(), 0, &mut apps);
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "Example");
    }
}

#[cfg(test)]
mod association_tests {
    use super::*;
    use crate::model::ApplicationIdentity;
    fn identity() -> ApplicationIdentity {
        ApplicationIdentity {
            platform: "macos".into(),
            value: "org.example.application".into(),
        }
    }
    #[tokio::test]
    async fn association_selection_returns_identity_name_and_optional_icon_together() {
        let association = association_with(
            async { Ok(identity()) },
            || "Example".into(),
            || async { Ok("data:image/png;base64,fixture".into()) },
        )
        .await
        .unwrap();
        assert_eq!(association.name, "Example");
        assert_eq!(association.identities, vec![identity()]);
        assert!(association.image.is_some());
        let association = association_with(
            async { Ok(identity()) },
            || "Example".into(),
            || async { Err("Icon unavailable".into()) },
        )
        .await
        .unwrap();
        assert_eq!(association.identities, vec![identity()]);
        assert!(association.image.is_none());
    }
    #[tokio::test]
    async fn failed_or_invalid_identity_does_not_request_icon_or_return_partial_selection() {
        for identity in [
            Err("Application no longer exists".into()),
            Ok(ApplicationIdentity {
                platform: "unknown".into(),
                value: "bad".into(),
            }),
        ] {
            let result = association_with(
                async { identity },
                || panic!("name lookup must not run"),
                || async { panic!("icon lookup must not run") },
            )
            .await;
            assert!(result.is_err());
        }
        assert!(association("/definitely-not-an-installed-app")
            .await
            .is_err());
    }
}
