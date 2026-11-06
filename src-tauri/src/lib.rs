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

struct AppSettings {
    store: SettingsStore,
    overrides: Arc<CliOverrides>,
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn scan_sessions(catalog: tauri::State<'_, SharedCatalog>) -> Result<ScanResponse, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        let scans = catalog.scan();
        dto::scan_response(&catalog, scans, SystemTime::now())
    })
    .await
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
    uids: Vec<String>,
) -> Result<Vec<DeleteResult>, String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        catalog
            .delete(&uids, &SystemBackend, SystemTime::now())
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
async fn reveal_session_file(catalog: tauri::State<'_, SharedCatalog>, path: String) -> Result<(), String> {
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
async fn open_project_dir(catalog: tauri::State<'_, SharedCatalog>, uid: String) -> Result<(), String> {
    let catalog = catalog.inner().clone();
    blocking(move || {
        let dir = catalog.project_dir(&uid)?;
        tauri_plugin_opener::open_path(&dir, None::<&str>).map_err(|error| error.to_string())
    })
    .await?
}

#[tauri::command]
async fn load_transcript(catalog: tauri::State<'_, SharedCatalog>, uid: String) -> Result<TranscriptDto, String> {
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
fn set_theme(settings: tauri::State<'_, AppSettings>, theme: settings::Theme) -> Result<(), String> {
    settings.store.update(|current| current.theme = theme).map(|_| ())
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
            load_transcript,
            cli_statuses,
            set_cli_path,
            get_theme,
            set_theme
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
