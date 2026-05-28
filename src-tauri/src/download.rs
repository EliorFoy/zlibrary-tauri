use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use futures_util::StreamExt;

use crate::client;
use crate::log_info;
use crate::model::BookInfo;

const CHUNK_SIZE: u64 = 1024 * 1024;
const MAX_CONCURRENT: usize = 8;

pub trait ProgressCallback: Send + Sync + 'static {
    fn on_start(&self, total_bytes: u64);
    fn on_progress(&self, downloaded: u64, total: u64);
    fn on_finish(&self);
}

struct NoopProgress;
impl ProgressCallback for NoopProgress {
    fn on_start(&self, _: u64) {}
    fn on_progress(&self, _: u64, _: u64) {}
    fn on_finish(&self) {}
}

static DOWNLOAD_CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(|| {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::USER_AGENT,
            reqwest::header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
            ),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            reqwest::header::HeaderValue::from_static("*/*"),
        );
        headers.insert(
            reqwest::header::REFERER,
            reqwest::header::HeaderValue::from_static("https://z-library.sk/"),
        );
        headers.insert(
            reqwest::header::ORIGIN,
            reqwest::header::HeaderValue::from_static("https://z-library.sk"),
        );
        headers.insert(
            "sec-fetch-dest",
            reqwest::header::HeaderValue::from_static("empty"),
        );
        headers.insert(
            "sec-fetch-mode",
            reqwest::header::HeaderValue::from_static("cors"),
        );
        headers.insert(
            "sec-fetch-site",
            reqwest::header::HeaderValue::from_static("cross-site"),
        );
        reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .default_headers(headers)
            .timeout(std::time::Duration::from_secs(600))
            .build()
            .expect("Failed to build download client")
    });

pub async fn download_book(book: &BookInfo) -> Result<PathBuf, String> {
    download_book_with_progress(book, Arc::new(NoopProgress), None).await
}

pub async fn download_book_with_progress(
    book: &BookInfo,
    progress: Arc<dyn ProgressCallback>,
    account: Option<(&str, &str)>,  // Some((user_id, user_key))
) -> Result<PathBuf, String> {
    let url = &book.download_url;
    if url.is_empty() {
        return Err("下载链接为空".into());
    }

    let rewritten = client::rewrite_url(url);
    let filename = build_filename(book);
    let download_dir = crate::paths::downloads_dir()?;
    let save_path = download_dir.join(&filename);
    if let Some(parent) = save_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("创建下载目录失败: {e}"))?;
    }

    let redirect_url = follow_redirect(&rewritten, account).await?;

    fetch_with_range(&redirect_url, &save_path, progress).await?;

    Ok(save_path)
}

async fn fetch_with_range(
    url: &str,
    save_path: &PathBuf,
    progress: Arc<dyn ProgressCallback>,
) -> Result<(), String> {
    let head_resp = DOWNLOAD_CLIENT
        .head(url)
        .send()
        .await
        .map_err(|e| format!("HEAD 请求失败: {e}"))?;

    let total_size: u64 = head_resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);

    let accept_ranges = head_resp
        .headers()
        .get("accept-ranges")
        .map(|v| v.as_bytes() == b"bytes")
        .unwrap_or(false);

    if total_size > CHUNK_SIZE && accept_ranges {
        return fetch_chunked(url, save_path, total_size, progress).await;
    }

    fetch_streaming(url, save_path, total_size, progress).await
}

async fn fetch_streaming(
    url: &str,
    save_path: &PathBuf,
    total_size: u64,
    progress: Arc<dyn ProgressCallback>,
) -> Result<(), String> {
    let resp = DOWNLOAD_CLIENT
        .get(url)
        .send()
        .await
        .map_err(|e| format!("下载请求失败: {e}"))?;

    let status = resp.status();
    if !status.is_success() {
        return Err(format!("下载失败 HTTP {status}"));
    }

    let actual_size = resp
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    let size = actual_size.unwrap_or(total_size);
    if size > 0 {
        progress.on_start(size);
    }

    let mut file = std::fs::File::create(save_path)
        .map_err(|e| format!("创建文件失败: {e}"))?;

    let mut stream = resp.bytes_stream();
    let mut downloaded: u64 = 0;

    while let Some(chunk) = stream.next().await {
        let bytes = chunk.map_err(|e| format!("读取数据失败: {e}"))?;
        file.write_all(&bytes)
            .map_err(|e| format!("写入文件失败: {e}"))?;
        downloaded += bytes.len() as u64;
        if size > 0 {
            progress.on_progress(downloaded, size);
        }
    }

    file.flush().map_err(|e| format!("刷新文件失败: {e}"))?;
    progress.on_finish();
    Ok(())
}

async fn fetch_chunked(
    url: &str,
    save_path: &PathBuf,
    total_size: u64,
    progress: Arc<dyn ProgressCallback>,
) -> Result<(), String> {
    progress.on_start(total_size);

    let chunk_count = ((total_size + CHUNK_SIZE - 1) / CHUNK_SIZE)
        .min(MAX_CONCURRENT as u64)
        .max(1);

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(save_path)
        .map_err(|e| format!("创建文件失败: {e}"))?;

    file.set_len(total_size)
        .map_err(|e| format!("设置文件大小失败: {e}"))?;

    let file = Arc::new(std::sync::Mutex::new(file));
    let downloaded = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let errors = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    for i in 0..chunk_count {
        let start = i * (total_size / chunk_count);
        let end = if i == chunk_count - 1 {
            total_size - 1
        } else {
            (i + 1) * (total_size / chunk_count) - 1
        };

        let url = url.to_string();
        let file = file.clone();
        let downloaded = downloaded.clone();
        let errors = errors.clone();
        let progress = progress.clone();

        handles.push(tokio::spawn(async move {
            let result = download_chunk(&url, start, end, &file).await;
            match result {
                Ok(bytes) => {
                    let total =
                        downloaded.fetch_add(bytes, std::sync::atomic::Ordering::SeqCst) + bytes;
                    progress.on_progress(total, total_size);
                }
                Err(e) => {
                    errors.lock().unwrap().push(format!("分片 {start}-{end}: {e}"));
                }
            }
        }));
    }

    for h in handles {
        let _ = h.await;
    }

    let errs = errors.lock().unwrap();
    if !errs.is_empty() {
        return Err(errs.join("; "));
    }

    progress.on_finish();
    Ok(())
}

async fn download_chunk(
    url: &str,
    start: u64,
    end: u64,
    file: &std::sync::Mutex<std::fs::File>,
) -> Result<u64, String> {
    let range_header = format!("bytes={start}-{end}");
    let resp = DOWNLOAD_CLIENT
        .get(url)
        .header("range", &range_header)
        .send()
        .await
        .map_err(|e| format!("分片请求失败: {e}"))?;

    let status = resp.status();
    if !status.is_success() && status.as_u16() != 206 {
        return Err(format!("分片 HTTP {status}"));
    }

    let data = resp
        .bytes()
        .await
        .map_err(|e| format!("分片读取失败: {e}"))?;

    let len = data.len() as u64;
    {
        let mut f = file.lock().unwrap();
        use std::io::Seek;
        f.seek(std::io::SeekFrom::Start(start))
            .map_err(|e| format!("文件定位失败: {e}"))?;
        f.write_all(&data)
            .map_err(|e| format!("文件写入失败: {e}"))?;
    }
    Ok(len)
}

async fn follow_redirect(url: &str, account: Option<(&str, &str)>) -> Result<String, String> {
    let resp = client::get_with_challenge_and_account(url, account).await?;
    let status = resp.status();

    if status.is_redirection() {
        let location = resp
            .headers()
            .get("location")
            .ok_or_else(|| format!("HTTP {status}: 重定向缺少 Location 头"))?
            .to_str()
            .map_err(|e| format!("Location 头解析失败: {e}"))?;
        log_info!("[redirect] {status} -> {location}");
        return Ok(location.to_string());
    }

    if status.is_success() {
        let final_url = resp.url().to_string();
        if final_url != url {
            log_info!("[redirect] 200 -> {final_url}");
            return Ok(final_url);
        }
        let body = resp.text().await.map_err(|e| format!("读取页面失败: {e}"))?;
        let dl_link = extract_download_link(&body)
            .ok_or_else(|| "无法从页面解析下载链接".to_string())?;
        return Ok(dl_link);
    }

    Err(format!("HTTP {status}: 无法获取下载地址"))
}

fn extract_download_link(html: &str) -> Option<String> {
    let re = regex::Regex::new(
        r#"href="(https?://[^"]+)"[^>]*>\s*(?:Download|下载|GET|get)\b"#,
    )
    .ok()?;
    if let Some(cap) = re.captures(html) {
        return Some(cap[1].to_string());
    }
    let re2 = regex::Regex::new(
        r#"<a[^>]+href="(https?://[^"]+)"[^>]*class="[^"]*dlButton[^"]*""#,
    )
    .ok()?;
    if let Some(cap) = re2.captures(html) {
        return Some(cap[1].to_string());
    }
    None
}

fn build_filename(book: &BookInfo) -> String {
    let mut name = String::new();
    if !book.title.is_empty() {
        let t = sanitize_filename(&book.title);
        name.push_str(&t);
    }
    if !book.author.is_empty() {
        let a = sanitize_filename(&book.author);
        if name.is_empty() {
            name = a;
        } else {
            name.push_str(" - ");
            name.push_str(&a);
        }
    }
    if name.is_empty() {
        name = format!("book_{}", &book.id);
    }
    let ext = if book.extension.is_empty() {
        "epub"
    } else {
        &book.extension
    };
    format!("{}.{}", name, ext)
}

fn sanitize_filename(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .take(120)
        .collect()
}
