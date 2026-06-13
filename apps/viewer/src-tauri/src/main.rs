// codemap viewer — Tauri v2 desktop shell.
// Scope: folder-pick -> analyze -> render. The webview is the existing Vite app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;

/// Run the extraction engine on `path` and return canonical IR JSON.
/// `Err` carries a readable message shown in the UI.
///
/// Extraction is CPU-bound and can take a while on a large repo, so it runs on a
/// blocking worker thread — the webview/UI thread never stalls during analysis.
#[tauri::command]
async fn analyze_repo(path: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        extract::extract_repo(Path::new(&path))
            .map(|doc| doc.to_canonical_json())
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("analysis task failed: {e}"))?
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![analyze_repo])
        .run(tauri::generate_context!())
        .expect("error while running codemap viewer");
}
