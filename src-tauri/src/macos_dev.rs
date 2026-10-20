//! Give unbundled macOS development launches their real application identity.
//! Embedded plist metadata alone does not set the Dock label of a raw executable.

use std::{
    io,
    os::unix::{fs::MetadataExt, process::CommandExt},
    path::{Path, PathBuf},
    process::Command,
};

const BUNDLE_ENV: &str = "OPENSTREAMDECK_DEV_BUNDLE_PID";

fn prepare_bundle(executable: &Path) -> io::Result<PathBuf> {
    let directory = executable
        .parent()
        .ok_or_else(|| io::Error::other("Development executable has no parent directory"))?;
    let contents = directory.join("OpenStreamDeck.app/Contents");
    let macos = contents.join("MacOS");
    std::fs::create_dir_all(&macos)?;
    std::fs::write(contents.join("Info.plist"), include_bytes!("../Info.plist"))?;
    let launcher = macos.join("openstreamdeck");
    let source = std::fs::metadata(executable)?;
    if std::fs::symlink_metadata(&launcher).is_ok_and(|existing| {
        existing.is_file() && existing.dev() == source.dev() && existing.ino() == source.ino()
    }) {
        return Ok(launcher);
    }
    // Tauri rejects symlinks anywhere in the starting executable path on macOS.
    // A hard link keeps the Dock bundle identity without copying the binary or
    // weakening that check. Refresh it when Cargo replaces the executable inode.
    let pending = macos.join(format!(".openstreamdeck-{}", std::process::id()));
    std::fs::hard_link(executable, &pending)?;
    if let Err(error) = std::fs::rename(&pending, &launcher) {
        let _ = std::fs::remove_file(&pending);
        return Err(error);
    }
    Ok(launcher)
}

pub fn enter_bundle() -> io::Result<()> {
    let pid = std::process::id().to_string();
    // exec preserves the PID; the guard does not affect separately spawned apps.
    if std::env::var(BUNDLE_ENV).as_deref() == Ok(pid.as_str()) {
        std::env::remove_var(BUNDLE_ENV);
        return Ok(());
    }
    // Resolve parent-directory aliases too: Tauri checks every path component.
    let executable = std::env::current_exe()?.canonicalize()?;
    if executable
        .parent()
        .is_some_and(|p| p.ends_with("Contents/MacOS"))
    {
        return Ok(());
    }
    let launcher = prepare_bundle(&executable)?;
    // Replace this process before Tauri, USB or background workers start. Keep
    // arguments, working directory, stdio and PID for Cargo's watcher/signals.
    Err(Command::new(launcher)
        .args(std::env::args_os().skip(1))
        .env(BUNDLE_ENV, pid)
        .exec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dev_bundle_reuses_current_executable_and_refreshes_metadata() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("openstreamdeck");
        std::fs::write(&executable, b"first build").unwrap();
        let launcher = prepare_bundle(&executable).unwrap();
        assert!(!std::fs::symlink_metadata(&launcher)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(
            std::fs::metadata(&launcher).unwrap().ino(),
            std::fs::metadata(&executable).unwrap().ino()
        );
        // Cargo replaces its output, so the bundle must follow the new inode.
        let replacement = dir.path().join("next-build");
        std::fs::write(&replacement, b"next build").unwrap();
        std::fs::rename(&replacement, &executable).unwrap();
        assert_eq!(std::fs::read(&launcher).unwrap(), b"first build");
        let plist = launcher
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Info.plist");
        std::fs::write(&plist, b"stale").unwrap();
        assert_eq!(prepare_bundle(&executable).unwrap(), launcher);
        assert_eq!(std::fs::read(&launcher).unwrap(), b"next build");
        assert_eq!(
            std::fs::read(plist).unwrap(),
            include_bytes!("../Info.plist")
        );
    }

    #[test]
    fn dev_bundle_reports_blocked_directory_without_overwriting_it() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("OpenStreamDeck.app");
        std::fs::write(&blocker, b"existing file").unwrap();
        assert!(prepare_bundle(&dir.path().join("openstreamdeck")).is_err());
        assert_eq!(std::fs::read(blocker).unwrap(), b"existing file");
    }

    #[test]
    fn dev_bundle_replaces_an_existing_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("openstreamdeck");
        std::fs::write(&executable, b"development binary").unwrap();
        let macos = dir.path().join("OpenStreamDeck.app/Contents/MacOS");
        std::fs::create_dir_all(&macos).unwrap();
        let launcher = macos.join("openstreamdeck");
        std::os::unix::fs::symlink(&executable, &launcher).unwrap();
        assert_eq!(prepare_bundle(&executable).unwrap(), launcher);
        assert!(!std::fs::symlink_metadata(&launcher)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read(&launcher).unwrap(), b"development binary");
    }

    #[test]
    fn bundled_subprocess_passes_tauri_starting_binary_validation() {
        const CHILD: &str = "OPENSTREAMDECK_TEST_BUNDLED_EXECUTABLE";
        if std::env::var_os(CHILD).is_some() {
            let executable = tauri::utils::platform::current_exe()
                .expect("Tauri must accept the development bundle executable");
            assert!(executable.parent().unwrap().ends_with("Contents/MacOS"));
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        // macOS /var and /tmp may themselves be aliases; use the real directory.
        let executable = dir.path().canonicalize().unwrap().join("test-binary");
        std::fs::hard_link(std::env::current_exe().unwrap(), &executable).unwrap();
        let launcher = prepare_bundle(&executable).unwrap();
        let mut child = Command::new(launcher)
            .args([
                "--exact",
                "macos_dev::tests::bundled_subprocess_passes_tauri_starting_binary_validation",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .spawn()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "bundled test subprocess failed");
                break;
            }
            if std::time::Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("bundled test subprocess timed out");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}
