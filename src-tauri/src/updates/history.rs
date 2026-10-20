//! Small updater metadata file, separate from the editable page/settings configuration.
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Default, Deserialize, Serialize)]
pub(super) struct History {
    pub last_checked_at: Option<u64>,
    pub skipped_version: Option<String>,
}

pub(super) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(super) fn load(path: &Path) -> Result<History, String> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(History::default()),
        Err(error) => return Err(format!("Could not read update history: {error}")),
    };
    serde_json::from_slice(&bytes).map_err(|_| "Update history is malformed.".into())
}

pub(super) fn save(path: &Path, history: &History) -> Result<(), String> {
    let save = || -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let temp = path.with_extension("tmp");
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(&serde_json::to_vec(history)?)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(temp, path)?;
        Ok(())
    };
    save().map_err(|error| format!("Could not save update history: {error}"))
}
