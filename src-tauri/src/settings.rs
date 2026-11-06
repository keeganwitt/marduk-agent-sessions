use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// User-chosen CLI locations, keyed by agent id.
    #[serde(default)]
    pub cli_paths: BTreeMap<String, PathBuf>,
}

/// Settings persisted as JSON in the app's config folder.
pub struct SettingsStore {
    path: PathBuf,
    current: Mutex<Settings>,
}

impl SettingsStore {
    /// A missing or unreadable file starts from defaults rather than blocking the app.
    pub fn load(path: PathBuf) -> Self {
        let current = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        Self {
            path,
            current: Mutex::new(current),
        }
    }

    pub fn get(&self) -> Settings {
        self.current.lock().unwrap().clone()
    }

    pub fn update(&self, change: impl FnOnce(&mut Settings)) -> Result<Settings, String> {
        let mut current = self.current.lock().unwrap();
        let mut next = current.clone();
        change(&mut next);
        write_atomically(&self.path, &next)?;
        *current = next.clone();
        Ok(next)
    }
}

fn write_atomically(path: &Path, settings: &Settings) -> Result<(), String> {
    let dir = path.parent().ok_or("Settings path has no folder")?;
    std::fs::create_dir_all(dir).map_err(|error| format!("Couldn't create {}: {error}", dir.display()))?;
    let temporary = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(&temporary, json).map_err(|error| format!("Couldn't save settings: {error}"))?;
    std::fs::rename(&temporary, path).map_err(|error| format!("Couldn't save settings: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("settings.json");
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.get(), Settings::default());

        store
            .update(|settings| {
                settings.cli_paths.insert("codex".into(), PathBuf::from("/opt/codex"));
            })
            .unwrap();

        assert_eq!(
            SettingsStore::load(path).get().cli_paths.get("codex"),
            Some(&PathBuf::from("/opt/codex"))
        );
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ not json").unwrap();
        assert_eq!(SettingsStore::load(path).get(), Settings::default());
    }

    #[test]
    fn failed_save_keeps_previous_settings() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("file");
        std::fs::write(&blocker, "").unwrap();
        let store = SettingsStore::load(blocker.join("settings.json"));

        let result = store.update(|settings| {
            settings.cli_paths.insert("codex".into(), PathBuf::from("/x"));
        });

        assert!(result.is_err());
        assert!(store.get().cli_paths.is_empty());
    }
}
