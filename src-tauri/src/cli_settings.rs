use std::path::PathBuf;
use std::sync::Arc;

use marduk_core::CliOverrides;
use marduk_core::agents::cli::{self, CliInfo};
use marduk_core::catalog::Catalog;
use serde::Serialize;

use crate::settings::SettingsStore;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CliStatus {
    pub agent: &'static str,
    pub agent_name: &'static str,
    pub program: &'static str,
    pub install_url: &'static str,
    pub path: Option<String>,
    pub overridden: bool,
    pub version: Option<String>,
    pub problem: Option<String>,
}

fn status(agent: &'static str, agent_name: &'static str, info: CliInfo) -> CliStatus {
    let (version, problem) = match &info.path {
        None => (None, None),
        Some(path) => match cli::version(path) {
            Ok(version) => (Some(version), None),
            Err(error) => (None, Some(error)),
        },
    };
    CliStatus {
        agent,
        agent_name,
        program: info.program,
        install_url: info.install_url,
        path: info.path.map(|path| path.to_string_lossy().into_owned()),
        overridden: info.overridden,
        version,
        problem,
    }
}

pub fn statuses(catalog: &Catalog) -> Vec<CliStatus> {
    catalog
        .cli_status()
        .into_iter()
        .map(|(agent, name, info)| status(agent, name, info))
        .collect()
}

/// Saves (or, with `None`, clears) the CLI location for one agent after checking it.
pub fn set_path(
    catalog: &Catalog,
    overrides: &Arc<CliOverrides>,
    store: &SettingsStore,
    agent: &str,
    path: Option<String>,
) -> Result<Vec<CliStatus>, String> {
    if !catalog.cli_status().iter().any(|(id, _, _)| *id == agent) {
        return Err(format!("{agent} doesn't use a CLI."));
    }
    let path = path.map(PathBuf::from);
    if let Some(path) = &path {
        if !path.is_absolute() {
            return Err("Choose the CLI by its full path.".to_string());
        }
        if !cli::is_executable(path) {
            return Err(format!("{} isn't an executable file.", path.display()));
        }
    }
    store.update(|settings| match &path {
        Some(path) => {
            settings.cli_paths.insert(agent.to_string(), path.clone());
        }
        None => {
            settings.cli_paths.remove(agent);
        }
    })?;
    overrides.set(agent, path);
    Ok(statuses(catalog))
}

pub fn apply_saved(store: &SettingsStore, overrides: &CliOverrides) {
    for (agent, path) in store.get().cli_paths {
        overrides.set(&agent, Some(path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (tempfile::TempDir, Arc<CliOverrides>, Catalog, SettingsStore) {
        let dir = tempfile::tempdir().unwrap();
        let overrides = Arc::new(CliOverrides::default());
        let catalog = Catalog::new(
            marduk_core::default_adapters(overrides.clone()),
            marduk_core::RUNNING_WINDOW,
        );
        let store = SettingsStore::load(dir.path().join("settings.json"));
        (dir, overrides, catalog, store)
    }

    #[test]
    fn lists_only_agents_that_need_a_cli() {
        let (_dir, _overrides, catalog, _store) = setup();
        let agents: Vec<&str> = catalog.cli_status().iter().map(|(id, _, _)| *id).collect();
        assert_eq!(agents, vec!["codex", "opencode", "kilo"]);
    }

    #[test]
    fn rejects_unknown_agents_relative_paths_and_non_executables() {
        let (dir, overrides, catalog, store) = setup();
        let plain = dir.path().join("plain");
        std::fs::write(&plain, "").unwrap();

        assert!(
            set_path(
                &catalog,
                &overrides,
                &store,
                "claude",
                Some("/bin/sh".into())
            )
            .is_err()
        );
        assert!(set_path(&catalog, &overrides, &store, "codex", Some("codex".into())).is_err());
        let error = set_path(
            &catalog,
            &overrides,
            &store,
            "codex",
            Some(plain.to_string_lossy().into()),
        )
        .unwrap_err();
        assert!(error.contains("isn't an executable"));
        assert!(store.get().cli_paths.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn saves_applies_and_clears_a_cli_path() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, overrides, catalog, store) = setup();
        let fake = dir.path().join("codex");
        std::fs::write(&fake, "#!/bin/sh\necho codex-cli 9.9.9\n").unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let result = set_path(
            &catalog,
            &overrides,
            &store,
            "codex",
            Some(fake.to_string_lossy().into()),
        )
        .unwrap();

        let codex = result
            .iter()
            .find(|status| status.agent == "codex")
            .unwrap();
        assert_eq!(codex.path.as_deref(), Some(fake.to_str().unwrap()));
        assert!(codex.overridden);
        assert_eq!(codex.version.as_deref(), Some("codex-cli 9.9.9"));
        assert_eq!(store.get().cli_paths.get("codex"), Some(&fake));

        let reloaded = Arc::new(CliOverrides::default());
        apply_saved(
            &SettingsStore::load(dir.path().join("settings.json")),
            &reloaded,
        );
        assert_eq!(reloaded.get("codex"), Some(fake));

        let cleared = set_path(&catalog, &overrides, &store, "codex", None).unwrap();
        assert!(
            !cleared
                .iter()
                .find(|status| status.agent == "codex")
                .unwrap()
                .overridden
        );
        assert!(overrides.get("codex").is_none());
    }
}
