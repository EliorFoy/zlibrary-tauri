mod client;
mod download;
mod model;
mod search;

use model::{BookInfo, SearchResult};

#[tauri::command]
async fn search_books(query: String, page: u32) -> Result<SearchResult, String> {
    search::search_books(&query, page).await
}

#[tauri::command]
async fn download_book(book: BookInfo) -> Result<String, String> {
    let path = download::download_book(&book).await?;
    Ok(path.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![search_books, download_book])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}