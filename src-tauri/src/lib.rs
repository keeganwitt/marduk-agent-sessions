mod cli_settings;
mod dto;
mod settings;

use std::sync::Arc;
use std::time::SystemTime;

use marduk_core::catalog::Catalog;
use marduk_core::{CliOverrides, RUNNING_WINDOW, SystemBackend};
use tauri::Manager;

use cli_settings::CliStatus;
use dto::{ArchiveResult, DeleteResult, PreviewItem, ScanResponse, TranscriptDto};
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
) -> Result<Vec<DeleteResult>, String> {
    let catalog = catalog.inner().clone();
    let labels = labels.inner().clone();
    blocking(move || {
        let keys: std::collections::HashMap<String, String> = uids
            .iter()
            .filter_map(|uid| Some((uid.clone(), catalog.key_of(uid)?)))
            .collect();
        let outcomes = catalog.delete(&uids, &SystemBackend, SystemTime::now());
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
) -> Result<Vec<ArchiveResult>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        catalog
            .toggle_archive(&uids, &SystemBackend, SystemTime::now())
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
            let store = SettingsStore::load(app.path().app_config_dir()?.join("settings.json"));
            cli_settings::apply_saved(&store, &overrides);
            app.manage(AppSettings { store, overrides });
            let labels_path = app.path().app_data_dir()?.join("labels.db");
            let labels = marduk_core::labels::LabelStore::open(&labels_path).or_else(|error| {
                eprintln!("Labels are unavailable ({error}); using a temporary store.");
                marduk_core::labels::LabelStore::in_memory()
            })?;
            app.manage::<SharedLabels>(Arc::new(labels));
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
            set_cli_path,
            get_theme,
            set_theme,
            list_labels,
            create_label,
            update_label,
            delete_label,
            apply_labels
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
