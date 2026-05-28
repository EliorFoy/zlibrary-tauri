pub mod account_pool;
pub mod client;
pub mod download;
pub mod logger;
pub mod mail_receiver;
pub mod model;
pub mod paths;
pub mod search;
pub mod solver;

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => {
        $crate::logger::log(&format!($($arg)*))
    };
}

#[cfg(feature = "gui")]
use std::sync::Arc;
#[cfg(feature = "gui")]
use tokio::sync::Mutex;
#[cfg(feature = "gui")]
use account_pool::{AccountInfo, AccountPool};
#[cfg(feature = "gui")]
use download::ProgressCallback;
#[cfg(feature = "gui")]
use model::{BookInfo, SearchResult};

#[cfg(feature = "gui")]
struct DownloadProgress {
    app: tauri::AppHandle,
    last_pct: std::sync::atomic::AtomicU32,
    download_id: String,
}

#[cfg(feature = "gui")]
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

#[cfg(feature = "gui")]
#[tauri::command]
async fn search_books(query: String, page: u32) -> Result<SearchResult, String> {
    search::search_books(&query, page).await
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn download_book(
    book: BookInfo,
    download_id: String,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<String, String> {
    let progress = Arc::new(DownloadProgress {
        app: app_handle,
        last_pct: std::sync::atomic::AtomicU32::new(0),
        download_id,
    });

    let account_opt = {
        let pool = state.lock().await;
        pool.get_active_account()
            .or_else(|| pool.get_best_available_account())
    };

    let path = if let Some(account) = &account_opt {
        log_info!("[download] 使用账号 {} (剩余额度 {}) 下载", account.email, account.usage_count);

        let uid_str = account.user_id.to_string();
        let result = download::download_book_with_progress(
            &book,
            progress,
            Some((uid_str.as_str(), account.user_key.as_str())),
        ).await;

        if result.is_ok() {
            let pool = state.lock().await;
            if let Err(e) = pool.decrement_usage(account.id) {
                log_info!("[download] 扣减额度失败: {}", e);
            }
        }

        result?
    } else {
        log_info!("[download] 无可用账号，以游客模式下载");
        download::download_book_with_progress(&book, progress, None).await?
    };

    Ok(path.to_string_lossy().to_string())
}

#[cfg(feature = "gui")]
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

#[cfg(feature = "gui")]
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

#[cfg(feature = "gui")]
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

#[cfg(feature = "gui")]
#[tauri::command]
async fn open_file(path: String) -> Result<(), String> {
    let target = std::path::PathBuf::from(&path);
    if !target.exists() {
        return Err(format!("文件不存在: {}", target.display()));
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", &path])
            .spawn()
            .map_err(|e| format!("打开文件失败: {e}"))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("打开文件失败: {e}"))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&path)
            .spawn()
            .map_err(|e| format!("打开文件失败: {e}"))?;
    }

    #[cfg(target_os = "android")]
    {
        let _ = path;
        return Err("Android 暂不支持打开文件".to_string());
    }

    Ok(())
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn open_file_location(path: String) -> Result<(), String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = path;
        Err("当前移动平台不支持打开文件所在位置".to_string())
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let target = std::path::PathBuf::from(&path);
        log_info!("[open_file_location] path={path}, exists={}", target.exists());

        let dir = if target.is_dir() {
            target.clone()
        } else {
            match target.parent() {
                Some(p) => p.to_path_buf(),
                None => return Err(format!("无法获取父目录: {path}")),
            }
        };

        log_info!("[open_file_location] opening dir={}", dir.display());

        #[cfg(target_os = "windows")]
        {
            log_info!("[open_file_location] opening: {}", dir.display());
            std::process::Command::new("explorer")
                .arg(dir.as_os_str())
                .spawn()
                .map_err(|e| format!("打开资源管理器失败: {e}"))?;
            return Ok(());
        }

        #[cfg(target_os = "macos")]
        {
            let status = std::process::Command::new("open")
                .arg("-R")
                .arg(&target)
                .status()
                .map_err(|e| format!("打开 Finder 失败: {e}"))?;
            if status.success() {
                return Ok(());
            }

            return std::process::Command::new("open")
                .arg(&dir)
                .status()
                .map_err(|e| format!("打开 Finder 失败: {e}"))
                .and_then(|s| {
                    if s.success() {
                        Ok(())
                    } else {
                        Err(format!("Finder 返回失败状态: {s}"))
                    }
                });
        }

        #[cfg(target_os = "linux")]
        {
            return match std::process::Command::new("xdg-open").arg(&dir).status() {
                Ok(status) if status.success() => Ok(()),
                Ok(status) => {
                    let fallback = std::process::Command::new("gio")
                        .arg("open")
                        .arg(&dir)
                        .status();
                    match fallback {
                        Ok(fallback_status) if fallback_status.success() => Ok(()),
                        Ok(fallback_status) => Err(format!(
                            "打开目录失败: xdg-open={status}, gio={fallback_status}"
                        )),
                        Err(e) => Err(format!("打开目录失败: xdg-open={status}, gio={e}")),
                    }
                }
                Err(e) => {
                    let fallback = std::process::Command::new("gio")
                        .arg("open")
                        .arg(&dir)
                        .status();
                    match fallback {
                        Ok(fallback_status) if fallback_status.success() => Ok(()),
                        Ok(fallback_status) => {
                            Err(format!("打开目录失败: xdg-open={e}, gio={fallback_status}"))
                        }
                        Err(fallback_err) => {
                            Err(format!("打开目录失败: xdg-open={e}, gio={fallback_err}"))
                        }
                    }
                }
            };
        }

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        {
            Err("Unsupported platform".to_string())
        }
    }
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn list_accounts(
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<Vec<AccountInfo>, String> {
    let pool = state.lock().await;
    pool.list_accounts()
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn delete_account(
    id: i64,
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<(), String> {
    let pool = state.lock().await;
    pool.delete_account(id)
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn refresh_account_quota(
    id: i64,
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<i32, String> {
    let pool = state.lock().await;
    pool.refresh_account_quota(id).await
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn refresh_all_quotas(
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<Vec<(i64, String, i32)>, String> {
    let pool = state.lock().await;
    pool.refresh_all_quotas().await
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn set_active_account(
    id: i64,
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<(), String> {
    let pool = state.lock().await;
    pool.set_active_account(id)
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn get_active_account(
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<Option<AccountInfo>, String> {
    let pool = state.lock().await;
    Ok(pool.get_active_account())
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn check_download_available(
    state: tauri::State<'_, Mutex<AccountPool>>,
) -> Result<bool, String> {
    let pool = state.lock().await;
    Ok(pool.has_any_available_account())
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn load_download_history() -> Result<String, String> {
    let path = paths::download_history_file()?;
    if path.exists() {
        std::fs::read_to_string(&path).map_err(|e| format!("读取下载记录失败: {e}"))
    } else {
        Ok("[]".to_string())
    }
}

#[cfg(feature = "gui")]
#[tauri::command]
async fn save_download_history(data: String) -> Result<(), String> {
    let path = paths::download_history_file()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
    }
    std::fs::write(&path, &data).map_err(|e| format!("保存下载记录失败: {e}"))
}

#[cfg(feature = "gui")]
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            use tauri::Manager;

            paths::configure_tauri_paths(app);
            logger::init();
            let pool = AccountPool::new().map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("初始化账号数据库失败: {e}"),
                )
            })?;
            app.manage(Mutex::new(pool));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            search_books,
            download_book,
            manual_login,
            manual_register,
            send_registration_code,
            list_accounts,
            delete_account,
            open_file,
            open_file_location,
            refresh_account_quota,
            refresh_all_quotas,
            set_active_account,
            get_active_account,
            check_download_available,
            load_download_history,
            save_download_history,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
