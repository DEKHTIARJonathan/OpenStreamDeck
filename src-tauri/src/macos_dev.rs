//! Give unbundled macOS development launches their real application identity.
//! Embedded plist metadata alone does not set the Dock label of a raw executable.

use std::{
    io,
    os::unix::{fs::symlink, process::CommandExt},
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
    match std::fs::read_link(&launcher) {
        Ok(target) if target == executable => {}
        Ok(_) => {
            std::fs::remove_file(&launcher)?;
            symlink(executable, &launcher)?;
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            symlink(executable, &launcher)?;
        }
        Err(error) => return Err(error),
    }
    Ok(launcher)
}

pub fn enter_bundle() -> io::Result<()> {
    let pid = std::process::id().to_string();
    // exec preserves the PID. A PID-scoped guard also works when current_exe()
    // resolves the bundle's symlink, without affecting separately spawned apps.
    if std::env::var(BUNDLE_ENV).as_deref() == Ok(pid.as_str()) {
        std::env::remove_var(BUNDLE_ENV);
        return Ok(());
    }
    let executable = std::env::current_exe()?;
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
        assert_eq!(std::fs::read_link(&launcher).unwrap(), executable);
        std::fs::write(&executable, b"next build").unwrap();
        assert_eq!(std::fs::read(&launcher).unwrap(), b"next build");
        let plist = launcher
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Info.plist");
        std::fs::write(&plist, b"stale").unwrap();
        assert_eq!(prepare_bundle(&executable).unwrap(), launcher);
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
}
