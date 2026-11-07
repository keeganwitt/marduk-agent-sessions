use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Debug, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// User-chosen CLI locations, keyed by agent id.
    #[serde(default)]
    pub cli_paths: BTreeMap<String, PathBuf>,
    #[serde(default)]
    pub theme: Theme,
    /// `system`, or a UI language id such as `de` or `zh-Hans`; the UI validates it.
    #[serde(default = "system_language")]
    pub language: String,
    /// Column widths and visibility, as the UI stores them; Rust doesn't interpret them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<serde_json::Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cli_paths: BTreeMap::new(),
            theme: Theme::default(),
            language: system_language(),
            columns: None,
        }
    }
}

fn system_language() -> String {
    "system".to_string()
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
    std::fs::create_dir_all(dir)
        .map_err(|error| format!("Couldn't create {}: {error}", dir.display()))?;
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
                settings
                    .cli_paths
                    .insert("codex".into(), PathBuf::from("/opt/codex"));
            })
            .unwrap();

        assert_eq!(
            SettingsStore::load(path).get().cli_paths.get("codex"),
            Some(&PathBuf::from("/opt/codex"))
        );
    }

    #[test]
    fn language_defaults_to_system_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"cliPaths":{}}"#).unwrap();
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.get().language, "system");

        store
            .update(|settings| settings.language = "ja".into())
            .unwrap();

        assert_eq!(SettingsStore::load(path).get().language, "ja");
    }

    #[test]
    fn column_prefs_round_trip_and_default_to_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"cliPaths":{},"theme":"dark"}"#).unwrap();
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.get().columns, None);
        let prefs =
            serde_json::json!({ "widths": { "size": 120 }, "hidden": [], "shown": ["model"] });

        store
            .update(|settings| settings.columns = Some(prefs.clone()))
            .unwrap();

        let reloaded = SettingsStore::load(path).get();
        assert_eq!(reloaded.columns, Some(prefs));
        assert_eq!(reloaded.theme, Theme::Dark);
    }

    #[test]
    fn theme_defaults_to_system_and_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, r#"{"cliPaths":{}}"#).unwrap();
        let store = SettingsStore::load(path.clone());
        assert_eq!(store.get().theme, Theme::System);

        store
            .update(|settings| settings.theme = Theme::Dark)
            .unwrap();

        assert_eq!(SettingsStore::load(path.clone()).get().theme, Theme::Dark);
        assert!(
            std::fs::read_to_string(path)
                .unwrap()
                .contains(r#""theme": "dark""#)
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
            settings
                .cli_paths
                .insert("codex".into(), PathBuf::from("/x"));
        });

        assert!(result.is_err());
        assert!(store.get().cli_paths.is_empty());
    }
}
