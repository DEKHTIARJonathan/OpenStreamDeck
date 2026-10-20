use base64::{engine::general_purpose::STANDARD, Engine};
use std::{env, path::Path};

fn updater_public_key() -> Result<String, String> {
    const NAME: &str = "TAURI_SIGNING_PUBLIC_KEY";
    const NAMES: [&str; 3] = [
        NAME,
        "TAURI_SIGNING_PRIVATE_KEY",
        "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
    ];
    // Always watch the file, even when absent or overridden by the environment.
    println!("cargo:rerun-if-changed=../.env");
    for name in NAMES {
        println!("cargo:rerun-if-env-changed={name}");
    }
    if NAMES.iter().any(|name| env::var_os(name).is_none()) {
        let manifest_dir = env::var("CARGO_MANIFEST_DIR")
            .map_err(|_| "Cargo did not provide CARGO_MANIFEST_DIR.")?;
        let path = Path::new(&manifest_dir).join("../.env");
        match dotenvy::from_path_iter(path) {
            Ok(entries) => {
                for entry in entries {
                    // dotenv parse errors can contain the source line: never print them.
                    let (name, value) =
                        entry.map_err(|_| "Cannot parse root .env; check its dotenv syntax.")?;
                    if NAMES.contains(&name.as_str()) && env::var_os(&name).is_none() {
                        // Build scripts run in their own process. Never export private
                        // values with cargo:rustc-env or write them to generated config.
                        env::set_var(name, value);
                    }
                }
            }
            Err(dotenvy::Error::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {
                return Err(
                    "Cannot read root .env; check its permissions and dotenv syntax.".into(),
                )
            }
        }
    }
    for name in NAMES {
        let value = env::var(name).map_err(|_| {
            format!("Missing {name}. Set it in the environment or root .env; see .env.dist.")
        })?;
        if (name != "TAURI_SIGNING_PRIVATE_KEY_PASSWORD" && value.trim().is_empty())
            || value.starts_with("REPLACE_WITH_")
            || matches!(value.as_str(), "<key>" | "<password>")
        {
            return Err(format!(
                "Missing or placeholder {name}. Set it in the environment or root .env; see .env.dist."
            ));
        }
    }
    let key = env::var(NAME).expect("required variable validated above");
    let invalid = "Invalid TAURI_SIGNING_PUBLIC_KEY. Supply the single-line content of the Tauri .pub file, not a path or placeholder.";
    let decoded = STANDARD.decode(key.trim()).map_err(|_| invalid)?;
    let decoded = std::str::from_utf8(&decoded).map_err(|_| invalid)?;
    let mut lines = decoded.lines();
    if !lines
        .next()
        .is_some_and(|line| line.starts_with("untrusted comment: "))
    {
        return Err(invalid.into());
    }
    let bytes = STANDARD
        .decode(lines.next().ok_or(invalid)?)
        .map_err(|_| invalid)?;
    if bytes.len() != 42 || &bytes[..2] != b"Ed" || lines.next().is_some() {
        return Err(invalid.into());
    }
    Ok(key.trim().to_owned())
}

fn main() {
    let public_key =
        updater_public_key().unwrap_or_else(|error| panic!("Updater build configuration: {error}"));
    // Only the validated public key is embedded; private signing values stay with the CLI.
    println!("cargo:rustc-env=TAURI_SIGNING_PUBLIC_KEY={public_key}");
    // The dev executable embeds this metadata; rebuild when its display name changes.
    println!("cargo:rerun-if-changed=Info.plist");
    // Clear the CLI-injected environment override so the explicit runtime
    // policy below is authoritative on every platform.
    // This runs single-threaded, before tauri-build starts work.
    std::env::remove_var("STATIC_VCRUNTIME");
    let windows = if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // tauri-build's resource manifest reaches application binaries only.
        // Library unit tests also need Common Controls v6 to load Tauri's
        // dialog imports; otherwise Windows exits with 0xc0000139 before tests.
        // Embed it through the linker for every target instead. See:
        // https://github.com/tauri-apps/tauri/issues/13419
        let manifest =
            Path::new(&env::var("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"))
                .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        tauri_build::WindowsAttributes::new_without_app_manifest()
    } else {
        tauri_build::WindowsAttributes::new()
    };
    let attributes =
        tauri_build::Attributes::new().windows_attributes(windows.static_vc_runtime(true));
    tauri_build::try_build(attributes).expect("Tauri build configuration failed");
}
