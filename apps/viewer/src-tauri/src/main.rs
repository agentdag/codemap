// codemap viewer — Tauri v2 desktop shell.
// Scope: folder-pick -> analyze -> render. The webview is the existing Vite app.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::Path;

/// Run the extraction engine on `path` and return canonical IR JSON.
/// `Err` carries a readable message shown in the UI.
#[tauri::command]
fn analyze_repo(path: String) -> Result<String, String> {
    let doc = extract::extract_repo(Path::new(&path)).map_err(|e| e.to_string())?;
    Ok(doc.to_canonical_json())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![analyze_repo])
        .run(tauri::generate_context!())
        .expect("error while running codemap viewer");
}
