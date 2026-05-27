use std::sync::Arc;

use tokio::sync::Mutex;

use zlibrary_core::account_pool::{AccountInfo, AccountPool};
use zlibrary_core::download::{self, ProgressCallback};
use zlibrary_core::model::{BookInfo, SearchResult};
use zlibrary_core::search;

struct DownloadProgress {
    app: tauri::AppHandle,
    last_pct: std::sync::atomic::AtomicU32,
    download_id: String,
}

impl ProgressCallback for DownloadProgress {
    fn on_start(&self, total_bytes: u64) {
        use tauri::Emitter;
        let _ = self.app.emit("download-progress", serde_json::json!({
            "type": "start",
            "download_id": self.download_id,
            "total": total_bytes,
        }));
    }
    fn on_progress(&self, downloaded: u64, total: u64) {
        use tauri::Emitter;
        let pct = if total > 0 { (downloaded * 100 / total) as u32 } else { 0 };
        let prev = self.last_pct.swap(pct, std::sync::atomic::Ordering::Relaxed);
        if pct != prev {
            let _ = self.app.emit("download-progress", serde_json::json!({
                "type": "progress",
                "download_id": self.download_id,
                "downloaded": downloaded,
                "total": total,
            }));
        }
    }
    fn on_finish(&self) {
        use tauri::Emitter;
        let _ = self.app.emit("download-progress", serde_json::json!({
            "type": "finish",
            "download_id": self.download_id,
        }));
    }
}

#[tauri::command]
async fn search_books(query: String, page: u32) -> Result<SearchResult, String> {
    search::search_books(&query, page).await
}

#[tauri::command]
async fn download_book(book: BookInfo, download_id: String, app_handle: tauri::AppHandle) -> Result<String, String> {
    let progress = Arc::new(DownloadProgress {
        app: app_handle,
        last_pct: std::sync::atomic::AtomicU32::new(0),
        download_id,
    });
    let path = download::download_book_with_progress(&book, progress).await?;
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
async fn manual_login(
    email: String,
    password: String,
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<String, String> {
    let pool = state.lock().await;
    pool.manual_login(&email, &password).await?;
    Ok(email)
}

#[tauri::command]
async fn manual_register(
    email: String,
    password: String,
    name: String,
    code: String,
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<String, String> {
    let pool = state.lock().await;
    pool.manual_register(&email, &password, &name, &code).await?;
    Ok(email)
}

#[tauri::command]
async fn send_registration_code(
    email: String,
    password: String,
    name: String,
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<(), String> {
    let pool = state.lock().await;
    pool.send_code_for_email(&email, &password, &name).await
}

#[tauri::command]
async fn open_file_location(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg("/select,")
            .arg(&path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "macos")]
    {
        let p = std::path::Path::new(&path);
        let dir = p.parent().unwrap_or(p);
        std::process::Command::new("open")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        let p = std::path::Path::new(&path);
        let dir = p.parent().unwrap_or(p);
        std::process::Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
async fn list_accounts(
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<Vec<AccountInfo>, String> {
    let pool = state.lock().await;
    pool.list_accounts()
}

#[tauri::command]
async fn delete_account(
    id: i64,
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<(), String> {
    let pool = state.lock().await;
    pool.delete_account(id)
}

fn main() {
    run();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let pool = AccountPool::new().expect("初始化账号数据库失败");

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .manage(Mutex::new(pool))
        .invoke_handler(tauri::generate_handler![
            search_books,
            download_book,
            manual_login,
            manual_register,
            send_registration_code,
            list_accounts,
            delete_account,
            open_file_location,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
