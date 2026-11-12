mod cli_settings;
mod dto;
mod settings;

use std::sync::Arc;
use std::time::SystemTime;

use marduk_core::catalog::Catalog;
use marduk_core::{CliOverrides, RUNNING_WINDOW, SystemBackend};
use tauri::Manager;

use cli_settings::CliStatus;
use dto::{
    AgentHelpDto, ArchiveResult, DeleteResult, MemoryStoreDto, PreviewItem, ProjectPurgeDto,
    ScanResponse, TranscriptDto,
};
use settings::SettingsStore;

type SharedCatalog = Arc<Catalog>;
type SharedLabels = Arc<marduk_core::labels::LabelStore>;

struct AppSettings {
    store: SettingsStore,
    overrides: Arc<CliOverrides>,
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn scan_sessions(
    catalog: tauri::State<'_, SharedCatalog>,
    labels: tauri::State<'_, SharedLabels>,
    on_progress: tauri::ipc::Channel<dto::ScanProgressDto>,
) -> Result<ScanResponse, String> {
    let catalog = catalog.inner().clone();
    let labels = labels.inner().clone();
    blocking(move || {
        let scans = catalog.scan_with_progress(&|status| {
            let _ = on_progress.send(dto::scan_progress(status));
        });
        let now = SystemTime::now();
        let mut response = dto::scan_response(&catalog, scans, now);
        dto::attach_labels(&mut response, &labels, now)?;
        Ok(response)
    })
    .await?
}

#[tauri::command]
async fn list_labels(labels: tauri::State<'_, SharedLabels>) -> Result<Vec<dto::LabelDto>, String> {
    let labels = labels.inner().clone();
    blocking(move || Ok(labels.labels()?.into_iter().map(Into::into).collect())).await?
}

#[tauri::command]
async fn create_label(
    labels: tauri::State<'_, SharedLabels>,
    name: String,
    color: String,
) -> Result<dto::LabelDto, String> {
    let labels = labels.inner().clone();
    blocking(move || labels.create(&name, &color).map(Into::into)).await?
}

#[tauri::command]
async fn update_label(
    labels: tauri::State<'_, SharedLabels>,
    id: i64,
    name: String,
    color: String,
) -> Result<(), String> {
    let labels = labels.inner().clone();
    blocking(move || labels.update(id, &name, &color)).await?
}

#[tauri::command]
async fn delete_label(labels: tauri::State<'_, SharedLabels>, id: i64) -> Result<(), String> {
    let labels = labels.inner().clone();
    blocking(move || labels.delete(id)).await?
}

/// Sessions are named by uid, like every other command, and resolved to their stable key.
#[tauri::command]
async fn apply_labels(
    catalog: tauri::State<'_, SharedCatalog>,
    labels: tauri::State<'_, SharedLabels>,
    uids: Vec<String>,
    add: Vec<i64>,
    remove: Vec<i64>,
) -> Result<(), String> {
    let catalog = catalog.inner().clone();
    let labels = labels.inner().clone();
    blocking(move || {
        let keys: Vec<String> = uids.iter().filter_map(|uid| catalog.key_of(uid)).collect();
        labels.apply(&keys, &add, &remove, SystemTime::now())
    })
    .await?
}

#[tauri::command]
async fn preview_delete(
    catalog: tauri::State<'_, SharedCatalog>,
    uids: Vec<String>,
) -> Result<Vec<PreviewItem>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        catalog
            .preview(&uids, SystemTime::now())
            .into_iter()
            .map(|preview| dto::preview_item(&catalog, preview))
            .collect()
    })
    .await
}

#[tauri::command]
async fn delete_sessions(
    catalog: tauri::State<'_, SharedCatalog>,
    labels: tauri::State<'_, SharedLabels>,
    uids: Vec<String>,
    on_progress: tauri::ipc::Channel<dto::TaskProgressDto>,
) -> Result<Vec<DeleteResult>, String> {
    let catalog = catalog.inner().clone();
    let labels = labels.inner().clone();
    blocking(move || {
        let keys: std::collections::HashMap<String, String> = uids
            .iter()
            .filter_map(|uid| Some((uid.clone(), catalog.key_of(uid)?)))
            .collect();
        let outcomes =
            catalog.delete_with_progress(&uids, &SystemBackend, SystemTime::now(), &|progress| {
                let _ = on_progress.send(progress.into());
            });
        let deleted: Vec<String> = outcomes
            .iter()
            .filter(|(_, outcome)| {
                matches!(
                    outcome,
                    marduk_core::DeleteOutcome::Trashed { .. }
                        | marduk_core::DeleteOutcome::DeletedByAgent { .. }
                )
            })
            .filter_map(|(uid, _)| keys.get(uid).cloned())
            .collect();
        let _ = labels.forget(&deleted);
        outcomes
            .into_iter()
            .map(|(uid, outcome)| dto::delete_result(uid, outcome))
            .collect()
    })
    .await
}

#[tauri::command]
async fn toggle_archive(
    catalog: tauri::State<'_, SharedCatalog>,
    uids: Vec<String>,
    on_progress: tauri::ipc::Channel<dto::TaskProgressDto>,
) -> Result<Vec<ArchiveResult>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        catalog
            .toggle_archive_with_progress(&uids, &SystemBackend, SystemTime::now(), &|progress| {
                let _ = on_progress.send(progress.into());
            })
            .into_iter()
            .map(|(uid, outcome)| dto::archive_result(uid, outcome))
            .collect()
    })
    .await
}

#[tauri::command]
async fn reveal_session_file(
    catalog: tauri::State<'_, SharedCatalog>,
    path: String,
) -> Result<(), String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        let path = std::path::PathBuf::from(path);
        if !catalog.owns_path(&path) {
            return Err("That file isn't part of a scanned session.".to_string());
        }
        tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|error| error.to_string())
    })
    .await?
}

#[tauri::command]
async fn list_memory(
    catalog: tauri::State<'_, SharedCatalog>,
) -> Result<Vec<MemoryStoreDto>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        catalog
            .memory()
            .into_iter()
            .map(MemoryStoreDto::from)
            .collect()
    })
    .await
}

#[tauri::command]
async fn memory_left_behind(
    catalog: tauri::State<'_, SharedCatalog>,
    uids: Vec<String>,
) -> Result<Vec<MemoryStoreDto>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        catalog
            .memory_left_behind(&uids)
            .into_iter()
            .map(MemoryStoreDto::from)
            .collect()
    })
    .await
}

#[tauri::command]
async fn purges_left_behind(
    catalog: tauri::State<'_, SharedCatalog>,
    uids: Vec<String>,
) -> Result<Vec<ProjectPurgeDto>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        catalog
            .purges_left_behind(&uids)
            .into_iter()
            .map(|purge| dto::project_purge(&catalog, purge))
            .collect()
    })
    .await
}

/// Permanently purges a project with its agent's own command; refused while it has sessions.
#[tauri::command]
async fn purge_project(
    catalog: tauri::State<'_, SharedCatalog>,
    agent: String,
    folder: String,
) -> Result<String, String> {
    let catalog = catalog.inner().clone();
    blocking(move || catalog.purge_project(&agent, std::path::Path::new(&folder), &SystemBackend))
        .await?
}

#[tauri::command]
async fn read_memory_file(
    catalog: tauri::State<'_, SharedCatalog>,
    path: String,
) -> Result<String, String> {
    let catalog = catalog.inner().clone();
    blocking(move || catalog.read_memory_file(std::path::Path::new(&path))).await?
}

#[tauri::command]
async fn trash_memory(
    catalog: tauri::State<'_, SharedCatalog>,
    folder: String,
    only_if_unused: bool,
) -> Result<(), String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        catalog.trash_memory(
            std::path::Path::new(&folder),
            only_if_unused,
            &SystemBackend,
        )
    })
    .await?
}

/// Shows a memory store's folder, or one of its files, in the file manager.
#[tauri::command]
async fn reveal_memory(
    catalog: tauri::State<'_, SharedCatalog>,
    path: String,
) -> Result<(), String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        let path = std::path::PathBuf::from(path);
        let known = catalog
            .memory()
            .iter()
            .any(|store| store.folder == path || store.files.iter().any(|file| file.path == path));
        if !known {
            return Err("That isn't agent memory Marduk found.".to_string());
        }
        tauri_plugin_opener::reveal_item_in_dir(&path).map_err(|error| error.to_string())
    })
    .await?
}

#[tauri::command]
async fn file_details(
    catalog: tauri::State<'_, SharedCatalog>,
    paths: Vec<String>,
) -> Result<Vec<dto::FileDetail>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || dto::file_details(&catalog, &paths)).await
}

#[tauri::command]
async fn open_project_dir(
    catalog: tauri::State<'_, SharedCatalog>,
    uid: String,
) -> Result<(), String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        let dir = catalog.project_dir(&uid)?;
        tauri_plugin_opener::open_path(&dir, None::<&str>).map_err(|error| error.to_string())
    })
    .await?
}

#[tauri::command]
async fn load_transcript(
    catalog: tauri::State<'_, SharedCatalog>,
    uid: String,
) -> Result<TranscriptDto, String> {
    let catalog = catalog.inner().clone();
    blocking(move || catalog.transcript(&uid).map(dto::transcript_dto)).await?
}

#[tauri::command]
async fn cli_statuses(catalog: tauri::State<'_, SharedCatalog>) -> Result<Vec<CliStatus>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || cli_settings::statuses(&catalog)).await
}

#[tauri::command]
fn agent_help(catalog: tauri::State<'_, SharedCatalog>) -> Vec<AgentHelpDto> {
    catalog
        .agent_help()
        .into_iter()
        .map(AgentHelpDto::from)
        .collect()
}

#[tauri::command]
async fn set_cli_path(
    app: tauri::AppHandle,
    catalog: tauri::State<'_, SharedCatalog>,
    agent: String,
    path: Option<String>,
) -> Result<Vec<CliStatus>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        let settings = app.state::<AppSettings>();
        cli_settings::set_path(&catalog, &settings.overrides, &settings.store, &agent, path)
    })
    .await?
}

#[tauri::command]
fn get_ui_pref(settings: tauri::State<'_, AppSettings>, key: String) -> Option<serde_json::Value> {
    settings.store.get().ui.get(&key).cloned()
}

#[tauri::command]
fn set_ui_pref(
    settings: tauri::State<'_, AppSettings>,
    key: String,
    value: serde_json::Value,
) -> Result<(), String> {
    settings
        .store
        .update(|current| {
            current.ui.insert(key, value);
        })
        .map(|_| ())
}

#[tauri::command]
fn get_language(settings: tauri::State<'_, AppSettings>) -> String {
    settings.store.get().language
}

#[tauri::command]
fn set_language(settings: tauri::State<'_, AppSettings>, language: String) -> Result<(), String> {
    settings
        .store
        .update(|current| current.language = language)
        .map(|_| ())
}

#[tauri::command]
fn get_columns(settings: tauri::State<'_, AppSettings>) -> Option<serde_json::Value> {
    settings.store.get().columns
}

#[tauri::command]
fn set_columns(
    settings: tauri::State<'_, AppSettings>,
    columns: serde_json::Value,
) -> Result<(), String> {
    settings
        .store
        .update(|current| current.columns = Some(columns))
        .map(|_| ())
}

#[tauri::command]
fn get_theme(settings: tauri::State<'_, AppSettings>) -> settings::Theme {
    settings.store.get().theme
}

#[tauri::command]
fn set_theme(
    settings: tauri::State<'_, AppSettings>,
    theme: settings::Theme,
) -> Result<(), String> {
    settings
        .store
        .update(|current| current.theme = theme)
        .map(|_| ())
}

const DEV_TITLE: &str = "Marduk (dev)";

/// Dev builds say so in the title, except on demo data (`MARDUK_HOME`), which is used for
/// screenshots of the app as people will see it.
fn dev_title(debug_build: bool, demo_home: Option<&std::ffi::OsStr>) -> Option<&'static str> {
    (debug_build && demo_home.is_none()).then_some(DEV_TITLE)
}

/// The default size fits every default column; smaller screens get a window that fits them.
fn fit_to_screen(window: &tauri::WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = window.current_monitor()? else {
        return Ok(());
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area().size.to_logical::<f64>(scale);
    let size = window.outer_size()?.to_logical::<f64>(scale);
    if let Some((width, height)) = fitted_size((size.width, size.height), (area.width, area.height))
    {
        window.set_size(tauri::LogicalSize::new(width, height))?;
        window.center()?;
    }
    Ok(())
}

/// On a work area too small for the window, a size that fits it with a margin; `None` when
/// the window already fits.
fn fitted_size(window: (f64, f64), area: (f64, f64)) -> Option<(f64, f64)> {
    let fits = |size: f64, available: f64, margin: f64| {
        if size <= available {
            size
        } else {
            (available * margin).floor()
        }
    };
    let fitted = (fits(window.0, area.0, 0.95), fits(window.1, area.1, 0.92));
    (fitted != window).then_some(fitted)
}

/// `MARDUK_DATA_DIR` keeps settings and labels apart from the real ones, for the demo home.
fn storage_override(value: Option<std::ffi::OsString>) -> Option<std::path::PathBuf> {
    value
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let overrides = Arc::new(CliOverrides::default());
    let catalog: SharedCatalog = Arc::new(Catalog::new(
        marduk_core::default_adapters(overrides.clone()),
        RUNNING_WINDOW,
    ));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let (config_dir, data_dir) = match storage_override(std::env::var_os("MARDUK_DATA_DIR"))
            {
                Some(dir) => (dir.clone(), dir),
                None => (app.path().app_config_dir()?, app.path().app_data_dir()?),
            };
            let store = SettingsStore::load(config_dir.join("settings.json"));
            cli_settings::apply_saved(&store, &overrides);
            app.manage(AppSettings { store, overrides });
            let labels_path = data_dir.join("labels.db");
            let labels = marduk_core::labels::LabelStore::open(&labels_path).or_else(|error| {
                eprintln!("Labels are unavailable ({error}); using a temporary store.");
                marduk_core::labels::LabelStore::in_memory()
            })?;
            app.manage::<SharedLabels>(Arc::new(labels));
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = fit_to_screen(&window) {
                    eprintln!("Couldn't fit the window to the screen: {error}");
                }
                let demo_home = std::env::var_os("MARDUK_HOME");
                if let Some(title) = dev_title(cfg!(debug_assertions), demo_home.as_deref())
                    && let Err(error) = window.set_title(title)
                {
                    eprintln!("Couldn't set the window title: {error}");
                }
            }
            Ok(())
        })
        .manage(catalog)
        .invoke_handler(tauri::generate_handler![
            scan_sessions,
            preview_delete,
            delete_sessions,
            toggle_archive,
            reveal_session_file,
            open_project_dir,
            file_details,
            load_transcript,
            cli_statuses,
            agent_help,
            list_memory,
            memory_left_behind,
            purges_left_behind,
            purge_project,
            read_memory_file,
            trash_memory,
            reveal_memory,
            set_cli_path,
            get_theme,
            set_theme,
            get_columns,
            set_columns,
            get_language,
            set_language,
            get_ui_pref,
            set_ui_pref,
            list_labels,
            create_label,
            update_label,
            delete_label,
            apply_labels
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn titles_dev_builds_except_on_demo_data() {
        let demo = std::ffi::OsStr::new("/tmp/marduk-demo");

        assert_eq!(dev_title(true, None), Some("Marduk (dev)"));
        assert_eq!(dev_title(true, Some(demo)), None);
        assert_eq!(dev_title(false, None), None);
    }

    #[test]
    fn shrinks_the_window_only_on_screens_too_small_for_it() {
        assert_eq!(fitted_size((1360.0, 860.0), (1470.0, 920.0)), None);
        assert_eq!(
            fitted_size((1360.0, 860.0), (1280.0, 720.0)),
            Some((1216.0, 662.0))
        );
        assert_eq!(
            fitted_size((1360.0, 860.0), (1920.0, 800.0)),
            Some((1360.0, 736.0))
        );
    }

    #[test]
    fn marduk_data_dir_overrides_storage_unless_empty() {
        assert_eq!(
            storage_override(Some("demo/marduk".into())),
            Some(std::path::PathBuf::from("demo/marduk"))
        );
        assert_eq!(storage_override(Some("".into())), None);
        assert_eq!(storage_override(None), None);
    }
}
