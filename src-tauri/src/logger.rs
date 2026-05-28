use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Mutex;

static LOG: std::sync::LazyLock<Mutex<Option<BufWriter<File>>>> =
    std::sync::LazyLock::new(|| Mutex::new(None));

pub fn init() {
    let Ok(log_path) = crate::paths::log_file() else {
        return;
    };
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
