use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use crate::log_info;
use regex::Regex;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

pub const ORIGIN_DOMAIN: &str = "z-library.sk";
pub const URL_DOMAIN: &str = "Megumin";

static SESSION_COOKIES: std::sync::LazyLock<RwLock<HashMap<String, String>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn session_cookie_str() -> String {
    let mut parts = vec![
        "remix_userkey=a097500143c397d1c09c8c4c459bb142".to_string(),
        "remix_userid=35246529".to_string(),
        "selectedSiteMode=books".to_string(),
    ];
    let session = SESSION_COOKIES.read().unwrap();
    for (name, value) in session.iter() {
        parts.push(format!("{name}={value}"));
    }
    parts.join("; ")
}

fn update_session_from_challenge(cookie_str: &str, set_cookies: &[String]) {
    let mut session = SESSION_COOKIES.write().unwrap();
    for part in cookie_str.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            let k = k.trim();
            let v = v.trim();
            if !k.is_empty() && !v.is_empty()
                && (k == "c_token" || k == "c_time" || k == "bsrv")
            {
                session.insert(k.to_string(), v.to_string());
            }
        }
    }
    for sc in set_cookies {
        let clean = sc.split(';').next().unwrap_or("");
        if let Some((k, v)) = clean.split_once('=') {
            let k = k.trim();
            let v = v.trim();
            if !k.is_empty() && !v.is_empty() && (k == "bsrv" || k == "c_token" || k == "c_time") {
                session.insert(k.to_string(), v.to_string());
            }
        }
    }
    log_info!(
        "[session] cookies: {:?}",
        session.keys().collect::<Vec<_>>()
    );
}

pub static CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(|| build_client().expect("build_client"));

pub fn make_url(path: &str) -> String {
    format!("https://{}{}", URL_DOMAIN, path)
}

pub fn rewrite_url(url: &str) -> String {
    url.replace(ORIGIN_DOMAIN, URL_DOMAIN)
}

pub async fn warmup_session() -> Result<(), String> {
    if SESSION_COOKIES.read().unwrap().contains_key("c_token") {
        log_info!("[warmup] session 已有 c_token，跳过预热");
        return Ok(());
    }
    log_info!("[warmup] 预热 session，获取挑战 cookie...");
    let url = make_url("/");
    let _resp = get_with_challenge(&url).await?;
    log_info!("[warmup] session 预热完成");
    Ok(())
}

pub async fn get_with_challenge(url: &str) -> Result<reqwest::Response, String> {
    let resp = send_request_with_retry(url).await?;
    let status = resp.status();
    if status.as_u16() != 503 {
        return Ok(resp);
    }

    let set_cookie: Vec<String> = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect();

    let body = resp.text().await.map_err(|e| format!("读取响应失败: {e}"))?;
    if !body.contains("Checking your browser") {
        return send_request_with_retry(url).await;
    }

    log_info!("[challenge] 503 JS 挑战，破解中...");

    let challenge =
        crate::solver::parse_challenge(&body).ok_or("无法解析 JS 挑战 token")?;

    log_info!(
        "[challenge] token={}, offset={}",
        challenge.token, challenge.check_offset
    );

    let start = std::time::Instant::now();
    let solution = crate::solver::solve(&challenge);
    let elapsed = start.elapsed().as_millis() as u64;
    log_info!("[challenge] PoW: i={solution}, {elapsed}ms");

    let cookie = crate::solver::build_challenge_cookie(&challenge, solution, elapsed);
    let mut bsrv = String::new();
    for sc in &set_cookie {
        let clean: String = sc.split(';').next().unwrap_or("").to_string();
        if clean.starts_with("bsrv=") {
            bsrv = format!("; {clean}");
        }
    }
    let cookie = format!("{cookie}{bsrv}");

    update_session_from_challenge(&cookie, &set_cookie);

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::HOST,
        reqwest::header::HeaderValue::from_static(ORIGIN_DOMAIN),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
        ),
    );
    headers.insert(
        reqwest::header::COOKIE,
        reqwest::header::HeaderValue::from_str(&cookie).map_err(|e| e.to_string())?,
    );
    headers.insert(
        "sec-ch-ua",
        reqwest::header::HeaderValue::from_static(
            r#""Not A(Brand";v="99", "Google Chrome";v="121", "Chromium";v="121""#,
        ),
    );

    let client2 = reqwest::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .dns_resolver(Arc::new(DirectIpResolver::cached()))
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    log_info!("[challenge] cookie: {cookie}");

    client2
        .get(url)
        .send()
        .await
        .map_err(|e| format!("挑战后请求失败: {e}"))
}

async fn send_request_with_retry(url: &str) -> Result<reqwest::Response, String> {
    let cookie = session_cookie_str();
    let req = CLIENT.get(url).header(reqwest::header::COOKIE, &cookie);
    match req.send().await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            log_info!("[retry] 请求失败: {e}，刷新IP重试...");
            force_refresh_ip();
            let cookie = session_cookie_str();
            CLIENT
                .get(url)
                .header(reqwest::header::COOKIE, &cookie)
                .send()
                .await
                .map_err(|e2| {
                    let ip = get_resolved_ip();
                    format!(
                        "请求失败 (IP={ip}, SNI={URL_DOMAIN}, Host={ORIGIN_DOMAIN}): {e2}"
                    )
                })
        }
    }
}

fn ip_cache_path() -> PathBuf {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    exe_dir.join("ip_cache")
}

fn load_cached_ip() -> Option<Ipv4Addr> {
    let path = ip_cache_path();
    let content = fs::read_to_string(&path).ok()?;
    let ip_str = content.trim();
    ip_str.parse().ok()
}

fn save_ip_cache(ip: Ipv4Addr) {
    let path = ip_cache_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, ip.to_string());
}

fn resolve_from_diggui() -> Option<Ipv4Addr> {
    let client = reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(15))
        .build()
        .ok()?;

    let form = [
        ("type", "A"),
        ("hostname", ORIGIN_DOMAIN),
        ("nameserver", "public"),
        ("public", "8.8.8.8"),
        ("specify", ""),
        ("clientsubnet", ""),
        ("tcp", "def"),
        ("transport", "def"),
        ("mapped", "def"),
        ("nssearch", "def"),
        ("trace", "def"),
        ("recurse", "def"),
        ("edns", "def"),
        ("dnssec", "def"),
        ("subnet", "def"),
        ("cookie", "def"),
        ("all", "def"),
        ("cmd", "def"),
        ("question", "def"),
        ("answer", "def"),
        ("authority", "def"),
        ("additional", "def"),
        ("comments", "def"),
        ("stats", "def"),
        ("multiline", "def"),
        ("short", "def"),
        ("colorize", "on"),
    ];

    let resp = client
        .post("https://www.diggui.com/")
        .header(
            "user-agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36",
        )
        .form(&form)
        .send()
        .ok()?;

    let body = resp.text().ok()?;

    let re = Regex::new(&format!(
        r#"{}\.</a>\s+<span[^>]*>\d+</span>\s+<span[^>]*>IN</span>\s+<a[^>]*>A</a>\s+<a[^>]*>([0-9]{{1,3}}\.[0-9]{{1,3}}\.[0-9]{{1,3}}\.[0-9]{{1,3}})</a>"#,
        ORIGIN_DOMAIN.replace('.', "\\.")
    ))
    .ok()?;

    let ip_str = re.captures(&body)?.get(1)?.as_str();
    let ip: Ipv4Addr = ip_str.parse().ok()?;
    log_info!("[diggui] {ORIGIN_DOMAIN} -> {ip}");
    Some(ip)
}

fn resolve_system_dns(domain: &str) -> Option<Ipv4Addr> {
    format!("{domain}:443")
        .to_socket_addrs()
        .ok()?
        .find_map(|a| match a.ip() {
            IpAddr::V4(v4) => Some(v4),
            _ => None,
        })
}

pub fn init_resolver() {
    let ip = get_resolved_ip();
    log_info!("[init] z-library.sk -> {ip}");
}

pub fn get_resolved_ip() -> Ipv4Addr {
    CACHED_IP
        .read()
        .ok()
        .and_then(|ip| {
            if ip.is_unspecified() {
                None
            } else {
                Some(*ip)
            }
        })
        .unwrap_or_else(|| {
            let ip = load_cached_ip()
                .or_else(|| resolve_from_diggui())
                .or_else(|| resolve_system_dns(ORIGIN_DOMAIN))
                .unwrap_or(Ipv4Addr::new(176, 123, 7, 105));
            save_ip_cache(ip);
            if let Ok(mut c) = CACHED_IP.write() {
                *c = ip;
            }
            ip
        })
}

pub fn force_refresh_ip() {
    let new_ip = resolve_from_diggui()
        .or_else(|| resolve_system_dns(ORIGIN_DOMAIN))
        .unwrap_or(Ipv4Addr::new(176, 123, 7, 105));
    log_info!("[resolver] 刷新 IP: {ORIGIN_DOMAIN} -> {new_ip}");
    save_ip_cache(new_ip);
    if let Ok(mut c) = CACHED_IP.write() {
        *c = new_ip;
    }
}

static CACHED_IP: std::sync::LazyLock<RwLock<Ipv4Addr>> =
    std::sync::LazyLock::new(|| RwLock::new(Ipv4Addr::UNSPECIFIED));

#[derive(Debug, Clone)]
pub struct DirectIpResolver {
    ip: Ipv4Addr,
}

impl DirectIpResolver {
    pub fn cached() -> Self {
        Self {
            ip: get_resolved_ip(),
        }
    }
}

impl Resolve for DirectIpResolver {
    fn resolve(&self, _name: Name) -> Resolving {
        let addr = SocketAddr::new(IpAddr::V4(self.ip), 0);
        let addrs: Addrs = Box::new(std::iter::once(addr));
        Box::pin(std::future::ready(Ok(addrs)))
    }
}

fn build_client() -> Result<reqwest::Client, reqwest::Error> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::HOST,
        reqwest::header::HeaderValue::from_static(ORIGIN_DOMAIN),
    );
    headers.insert(
        "sec-ch-ua",
        reqwest::header::HeaderValue::from_static(
            r#""Not A(Brand";v="99", "Google Chrome";v="121", "Chromium";v="121""#,
        ),
    );
    headers.insert(
        "sec-ch-ua-platform",
        reqwest::header::HeaderValue::from_static(r#""Windows""#),
    );
    headers.insert(
        "sec-ch-ua-mobile",
        reqwest::header::HeaderValue::from_static("?0"),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
        ),
    );

    reqwest::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .dns_resolver(Arc::new(DirectIpResolver::cached()))
        .default_headers(headers)
        .timeout(Duration::from_secs(30))
        .build()
}