use std::path::{Path, PathBuf};
use std::sync::OnceLock;

const APP_DIR_NAME: &str = "com.zlibrary.noproxy";

static APP_DATA_DIR: OnceLock<PathBuf> = OnceLock::new();
static APP_CACHE_DIR: OnceLock<PathBuf> = OnceLock::new();
static APP_LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
static DOWNLOAD_DIR: OnceLock<PathBuf> = OnceLock::new();

#[cfg(feature = "gui")]
pub fn configure_tauri_paths<R: tauri::Runtime>(app: &tauri::App<R>) {
    use tauri::Manager;

    if let Ok(dir) = app
        .path()
        .app_local_data_dir()
        .or_else(|_| app.path().app_data_dir())
    {
        let _ = APP_DATA_DIR.set(dir);
    }
    if let Ok(dir) = app.path().app_cache_dir() {
        let _ = APP_CACHE_DIR.set(dir);
    }
    if let Ok(dir) = app.path().app_log_dir() {
        let _ = APP_LOG_DIR.set(dir);
    }
    if let Ok(dir) = app.path().download_dir() {
        let _ = DOWNLOAD_DIR.set(dir);
    }
}

pub fn account_db_path() -> Result<PathBuf, String> {
    let path = app_data_dir()?.join("zlibrary_accounts.db");
    migrate_legacy_sibling("zlibrary_accounts.db", &path);
    Ok(path)
}

pub fn ip_cache_file() -> Result<PathBuf, String> {
    Ok(app_cache_dir()?.join("ip_cache"))
}

pub fn log_file() -> Result<PathBuf, String> {
    Ok(app_log_dir()?.join("zlibrary.log"))
}

pub fn downloads_dir() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        push_existing(
            &mut candidates,
            APP_DATA_DIR.get().map(|p| p.join("downloads")),
        );
        push_existing(&mut candidates, DOWNLOAD_DIR.get().cloned());
        push_existing(&mut candidates, dirs::download_dir());
        if let Ok(data_dir) = app_data_dir() {
            candidates.push(data_dir.join("downloads"));
        }
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        push_existing(&mut candidates, DOWNLOAD_DIR.get().cloned());
        push_existing(&mut candidates, dirs::download_dir());
        if let Ok(data_dir) = app_data_dir() {
            candidates.push(data_dir.join("downloads"));
        }
    }

    candidates.push(std::env::temp_dir().join(APP_DIR_NAME).join("downloads"));

    writable_dir(candidates, "下载目录")
}

fn app_data_dir() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    push_existing(&mut candidates, APP_DATA_DIR.get().cloned());
    push_app_dir(&mut candidates, dirs::data_local_dir());
    push_app_dir(&mut candidates, dirs::data_dir());
    push_existing(
        &mut candidates,
        dirs::home_dir().map(|home| home.join(format!(".{APP_DIR_NAME}"))),
    );
    candidates.push(std::env::temp_dir().join(APP_DIR_NAME));

    writable_dir(candidates, "应用数据目录")
}

fn app_cache_dir() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    push_existing(&mut candidates, APP_CACHE_DIR.get().cloned());
    push_app_dir(&mut candidates, dirs::cache_dir());
    if let Ok(data_dir) = app_data_dir() {
        candidates.push(data_dir.join("cache"));
    }
    candidates.push(std::env::temp_dir().join(APP_DIR_NAME).join("cache"));

    writable_dir(candidates, "缓存目录")
}

fn app_log_dir() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();
    push_existing(&mut candidates, APP_LOG_DIR.get().cloned());
    push_existing(
        &mut candidates,
        dirs::data_local_dir().map(|dir| dir.join(APP_DIR_NAME).join("logs")),
    );
    if let Ok(cache_dir) = app_cache_dir() {
        candidates.push(cache_dir.join("logs"));
    }
    candidates.push(std::env::temp_dir().join(APP_DIR_NAME).join("logs"));

    writable_dir(candidates, "日志目录")
}

fn push_app_dir(candidates: &mut Vec<PathBuf>, base: Option<PathBuf>) {
    push_existing(candidates, base.map(|dir| dir.join(APP_DIR_NAME)));
}

fn push_existing(candidates: &mut Vec<PathBuf>, path: Option<PathBuf>) {
    if let Some(path) = path {
        if !candidates.iter().any(|p| p == &path) {
            candidates.push(path);
        }
    }
}

fn writable_dir(candidates: Vec<PathBuf>, label: &str) -> Result<PathBuf, String> {
    let mut failures = Vec::new();

    for dir in candidates {
        if dir.as_os_str().is_empty() {
            continue;
        }
        match ensure_writable_dir(&dir) {
            Ok(()) => return Ok(dir),
            Err(e) => failures.push(format!("{} ({e})", dir.display())),
        }
    }

    if failures.is_empty() {
        Err(format!("无法定位可写的{label}"))
    } else {
        Err(format!("无法找到可写的{label}: {}", failures.join("; ")))
    }
}

fn ensure_writable_dir(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let probe = dir.join(format!(".zlibrary-write-test-{}", std::process::id()));
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&probe)?;
    let _ = std::fs::remove_file(probe);
    Ok(())
}

fn migrate_legacy_sibling(file_name: &str, target: &Path) {
    if target.exists() {
        return;
    }

    let Some(legacy) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|dir| dir.join(file_name)))
    else {
        return;
    };

    if !legacy.is_file() || legacy == target {
        return;
    }

    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::copy(legacy, target);
}
