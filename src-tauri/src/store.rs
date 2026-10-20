use crate::model::Config;
use std::path::Path;
pub fn load(dir: &Path) -> Result<Config, String> {
    let p = dir.join("config.json");
    if !p.exists() {
        return Ok(Config::default());
    }
    let data = std::fs::read(p).map_err(|e| e.to_string())?;
    Config::decode(&data)
}

pub fn save(dir: &Path, w: &Config) -> Result<(), String> {
    use std::io::Write;
    w.validate()?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let p = dir.join("config.json");
    let tmp = dir.join("config.tmp");
    let bytes = serde_json::to_vec_pretty(w).map_err(|e| e.to_string())?;
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    f.write_all(&bytes)
        .and_then(|_| f.sync_all())
        .map_err(|e| e.to_string())?;
    if p.exists() {
        std::fs::copy(&p, dir.join("config.backup.json")).map_err(|e| e.to_string())?;
    }
    drop(f);
    std::fs::rename(tmp, p).map_err(|e| e.to_string())
}
/// Caller holds the config lock. Failed persistence must not mutate live state.
pub fn update(
    dir: &Path,
    current: &mut Config,
    edit: impl FnOnce(&mut Config) -> Result<(), String>,
) -> Result<(), String> {
    let mut next = current.clone();
    edit(&mut next)?;
    save(dir, &next)?;
    *current = next;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_trip_backup() {
        let t = tempfile::tempdir().unwrap();
        let mut w = Config::default();
        save(t.path(), &w).unwrap();
        w.brightness = 21;
        save(t.path(), &w).unwrap();
        assert_eq!(load(t.path()).unwrap().brightness, 21);
        let b: Config =
            serde_json::from_slice(&std::fs::read(t.path().join("config.backup.json")).unwrap())
                .unwrap();
        assert_eq!(b.brightness, 60);
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn failed_transaction_leaves_live_and_saved_state_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut config = Config::default();
        save(dir.path(), &config).unwrap();
        // Portable failure injection: the temporary-file path is a directory.
        std::fs::create_dir(dir.path().join("config.tmp")).unwrap();
        assert!(update(dir.path(), &mut config, |next| {
            next.brightness = 5;
            Ok(())
        })
        .is_err());
        assert_eq!(config.brightness, 60);
        assert_eq!(load(dir.path()).unwrap().brightness, 60);
    }
    #[test]
    fn unsupported_schema_is_rejected_without_rewriting_saved_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let config = Config {
            version: 1,
            ..Default::default()
        };
        let bytes = serde_json::to_vec(&config).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(load(dir.path()).unwrap_err(), "Unsupported config version");
        assert_eq!(std::fs::read(path).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[test]
    fn invalid_save_cannot_replace_valid_config() {
        let dir = tempfile::tempdir().unwrap();
        let w = Config::default();
        save(dir.path(), &w).unwrap();
        let mut bad = w;
        bad.active_page = "missing".into();
        assert!(save(dir.path(), &bad).is_err());
        assert_eq!(load(dir.path()).unwrap().active_page, "home");
    }
    #[test]
    fn malformed_file_is_reported_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.json"), "bad json").unwrap();
        assert!(load(dir.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("config.json")).unwrap(),
            "bad json"
        );
    }
}
