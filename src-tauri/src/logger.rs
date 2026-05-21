use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Mutex;

static LOG: std::sync::LazyLock<Mutex<Option<BufWriter<File>>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

pub fn init() {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let log_path = exe_dir.join("zlibrary.log");
    match File::create(&log_path) {
        Ok(f) => {
            let mut writer = BufWriter::new(f);
            let _ = writeln!(writer, "=== Z-Library NoProxy log ===");
            let _ = writer.flush();
            *LOG.lock().unwrap() = Some(writer);
        }
        Err(_) => {}
    }
}

pub fn log(msg: &str) {
    if let Ok(mut guard) = LOG.lock() {
        if let Some(ref mut writer) = *guard {
            let _ = writeln!(writer, "{}", msg);
            let _ = writer.flush();
        }
    }
}