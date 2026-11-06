mod dto;

use std::sync::Arc;
use std::time::SystemTime;

use marduk_core::catalog::Catalog;
use marduk_core::{RUNNING_WINDOW, SystemBackend};

use dto::{ArchiveResult, DeleteResult, PreviewItem, ScanResponse};

type SharedCatalog = Arc<Catalog>;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let catalog: SharedCatalog = Arc::new(Catalog::new(marduk_core::default_adapters(), RUNNING_WINDOW));
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(catalog)
        .invoke_handler(tauri::generate_handler![
            scan_sessions,
            preview_delete,
            delete_sessions,
            toggle_archive,
            reveal_session_file
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
