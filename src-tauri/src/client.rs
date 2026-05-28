use std::collections::HashMap;
use std::fs;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use std::error::Error as _;

use crate::log_info;
use http_body_util::BodyExt as _;
use regex::Regex;

pub const ORIGIN_DOMAIN: &str = "z-library.sk";
pub const SNI_DOMAIN: &str = "Megumin";

static SESSION_COOKIES: std::sync::LazyLock<RwLock<HashMap<String, String>>> =
    std::sync::LazyLock::new(|| RwLock::new(HashMap::new()));

pub fn session_cookie_str() -> String {
    let mut parts = vec![
        "selectedSiteMode=books".to_string(),
    ];
    let session = SESSION_COOKIES.read().unwrap();
    for (name, value) in session.iter() {
        parts.push(format!("{name}={value}"));
    }
    parts.join("; ")
}

pub fn session_cookie_str_with(user_id: &str, user_key: &str) -> String {
    let mut parts = vec![
        format!("remix_userid={}", user_id),
        format!("remix_userkey={}", user_key),
        "selectedSiteMode=books".to_string(),
    ];
    let session = SESSION_COOKIES.read().unwrap();
    for (name, value) in session.iter() {
        parts.push(format!("{name}={value}"));
    }
    parts.join("; ")
}

pub fn registration_cookie_str() -> String {
    let mut parts = vec![
        "siteLanguage=en".to_string(),
        "refuseChangeDomain=1".to_string(),
    ];
    let session = SESSION_COOKIES.read().unwrap();
    for (name, value) in session.iter() {
        parts.push(format!("{name}={value}"));
    }
    parts.join("; ")
}

pub fn verify_cookie_str() -> String {
    let session = SESSION_COOKIES.read().unwrap();
    if let Some(bsrv) = session.get("bsrv") {
        format!("bsrv={bsrv}")
    } else {
        registration_cookie_str()
    }
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

// ---------------------------------------------------------------------------
// hyper + rustls SNI bypass client
// ---------------------------------------------------------------------------

type HttpClient = hyper_util::client::legacy::Client<SniConnector, http_body_util::combinators::BoxBody<bytes::Bytes, hyper::Error>>;

pub static CLIENT: std::sync::LazyLock<SniBypassClient> =
    std::sync::LazyLock::new(|| SniBypassClient::new());

pub struct SniBypassClient {
    inner: HttpClient,
}

#[derive(Debug)]
pub struct Response {
    status: u16,
    headers: http::HeaderMap,
    body: Option<bytes::Bytes>,
    url: String,
}

impl Response {
    pub fn status(&self) -> http::StatusCode {
        http::StatusCode::from_u16(self.status).unwrap_or(http::StatusCode::INTERNAL_SERVER_ERROR)
    }

    pub fn headers(&self) -> &http::HeaderMap {
        &self.headers
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub async fn text(&mut self) -> Result<String, String> {
        let bytes = self.take_body().await?;
        String::from_utf8(bytes.to_vec()).map_err(|e| format!("UTF-8 解码失败: {e}"))
    }

    pub async fn bytes(&mut self) -> Result<bytes::Bytes, String> {
        self.take_body().await
    }

    async fn take_body(&mut self) -> Result<bytes::Bytes, String> {
        self.body
            .take()
            .ok_or_else(|| "响应体已被消费".to_string())
    }
}

pub struct RequestBuilder {
    client: HttpClient,
    method: http::Method,
    url: String,
    headers: http::HeaderMap,
    body: Option<bytes::Bytes>,
}

impl RequestBuilder {
    pub fn header(mut self, key: http::header::HeaderName, value: &str) -> Self {
        if let Ok(v) = http::HeaderValue::from_str(value) {
            self.headers.insert(key, v);
        }
        self
    }

    pub fn header_str(mut self, name: &str, value: &str) -> Self {
        if let (Ok(k), Ok(v)) = (
            http::header::HeaderName::from_bytes(name.as_bytes()),
            http::HeaderValue::from_str(value),
        ) {
            self.headers.insert(k, v);
        }
        self
    }

    pub fn header_map(mut self, extra: &http::HeaderMap) -> Self {
        for (k, v) in extra.iter() {
            self.headers.insert(k, v.clone());
        }
        self
    }

    pub fn body(mut self, body: impl Into<bytes::Bytes>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub fn form<T: serde::Serialize + ?Sized>(mut self, form: &T) -> Self {
        let encoded = serde_urlencoded::to_string(form).unwrap_or_default();
        self.body = Some(bytes::Bytes::from(encoded));
        self.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_static("application/x-www-form-urlencoded; charset=UTF-8"),
        );
        self
    }

    pub fn multipart(mut self, boundary: &str, body: bytes::Bytes) -> Self {
        self.body = Some(body);
        self.headers.insert(
            http::header::CONTENT_TYPE,
            http::HeaderValue::from_str(&format!("multipart/form-data; boundary={boundary}"))
                .unwrap(),
        );
        self
    }

    pub async fn send(self) -> Result<Response, String> {
        let mut req = hyper::Request::builder()
            .method(&self.method)
            .uri(&self.url);

        // Set default HOST header from URL if not already set
        if !self.headers.contains_key(http::header::HOST) {
            if let Ok(parsed) = url::Url::parse(&self.url) {
                if let Some(host) = parsed.host_str() {
                    req = req.header(http::header::HOST, host);
                }
            }
        }

        let body = self.body.unwrap_or_default();
        let req = req
            .body(http_body_util::combinators::BoxBody::new(
                http_body_util::Full::new(body).map_err(|never| match never {}),
            ))
            .map_err(|e| format!("构建请求失败: {e}"))?;

        // Merge default headers
        let mut final_headers = self.headers.clone();
        for (k, v) in &self.headers {
            final_headers.insert(k.clone(), v.clone());
        }

        let (mut parts, body) = req.into_parts();
        parts.headers.extend(final_headers);
        let req = hyper::Request::from_parts(parts, body);

        let resp = self
            .client
            .request(req)
            .await
            .map_err(|e| {
                if e.is_connect() {
                    let inner = e.source().map(|s| format!("{s}")).unwrap_or_default();
                    format!("连接失败: {inner}")
                } else {
                    format!("请求失败: {e}")
                }
            })?;

        let status = resp.status().as_u16();
        let headers = resp.headers().clone();
        let url = self.url.clone();

        let (_, body) = resp.into_parts();
        let collected = http_body_util::BodyExt::collect(body)
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;
        let body_bytes = collected.to_bytes();

        Ok(Response {
            status,
            headers,
            body: Some(body_bytes),
            url,
        })
    }
}

impl SniBypassClient {
    fn new() -> Self {
        let connector = SniConnector::new();
        let inner = hyper_util::client::legacy::Client::builder(hyper_util::rt::TokioExecutor::new())
            .pool_max_idle_per_host(4)
            .build(connector);
        Self { inner }
    }

    pub fn get(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            client: self.inner.clone(),
            method: http::Method::GET,
            url: url.to_string(),
            headers: http::HeaderMap::new(),
            body: None,
        }
    }

    pub fn post(&self, url: &str) -> RequestBuilder {
        RequestBuilder {
            client: self.inner.clone(),
            method: http::Method::POST,
            url: url.to_string(),
            headers: http::HeaderMap::new(),
            body: None,
        }
    }
}

type BoxError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
struct SniConnector {
    tls: tokio_rustls::TlsConnector,
}

impl SniConnector {
    fn new() -> Self {
        let tls_config = rustls::ClientConfig::builder()
            .dangerous()
            .with_custom_certificate_verifier(Arc::new(NoVerifier))
            .with_no_client_auth();

        Self {
            tls: tokio_rustls::TlsConnector::from(Arc::new(tls_config)),
        }
    }
}

// Wrapper to implement Connection for TlsStream
struct SniConnection(tokio_rustls::client::TlsStream<tokio::net::TcpStream>);

impl hyper::rt::Write for SniConnection {
    fn poll_write(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        buf: &[u8],
    ) -> std::task::Poll<Result<usize, std::io::Error>> {
        use tokio::io::AsyncWrite;
        std::pin::Pin::new(&mut self.0).poll_write(cx, buf)
    }

    fn poll_flush(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        use tokio::io::AsyncWrite;
        std::pin::Pin::new(&mut self.0).poll_flush(cx)
    }

    fn poll_shutdown(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        use tokio::io::AsyncWrite;
        std::pin::Pin::new(&mut self.0).poll_shutdown(cx)
    }
}

impl hyper::rt::Read for SniConnection {
    fn poll_read(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> std::task::Poll<Result<(), std::io::Error>> {
        use tokio::io::AsyncRead;
        let cap = buf.remaining();
        if cap == 0 {
            return std::task::Poll::Ready(Ok(()));
        }
        let mut tmp = vec![0u8; cap];
        let mut read_buf = tokio::io::ReadBuf::new(&mut tmp);
        match std::pin::Pin::new(&mut self.0).poll_read(cx, &mut read_buf)? {
            std::task::Poll::Ready(()) => {
                let n = read_buf.filled().len();
                if n > 0 {
                    buf.put_slice(read_buf.filled());
                }
                std::task::Poll::Ready(Ok(()))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl hyper_util::client::legacy::connect::Connection for SniConnection {
    fn connected(&self) -> hyper_util::client::legacy::connect::Connected {
        hyper_util::client::legacy::connect::Connected::new()
    }
}

impl tower::Service<http::Uri> for SniConnector {
    type Response = SniConnection;
    type Error = BoxError;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>,
    >;

    fn poll_ready(
        &mut self,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, uri: http::Uri) -> Self::Future {
        let tls = self.tls.clone();

        Box::pin(async move {
            let host = uri.host().ok_or("URI 缺少 host")?;
            let port = uri.port_u16().unwrap_or(443);

            let ip = if host == ORIGIN_DOMAIN {
                get_resolved_ip()
            } else {
                format!("{host}:{port}")
                    .to_socket_addrs()
                    .map_err(|e| format!("DNS 解析失败: {e}"))?
                    .find_map(|a| match a.ip() {
                        IpAddr::V4(v4) => Some(v4),
                        _ => None,
                    })
                    .ok_or(format!("无法解析 {host}"))?
            };

            let addr = SocketAddr::new(IpAddr::V4(ip), port);
            log_info!("[sni] TCP {host}:{port} -> {addr}");
            let tcp = tokio::net::TcpStream::connect(addr)
                .await
                .map_err(|e| format!("TCP 连接失败 ({addr}): {e}"))?;
            tcp.set_nodelay(true).ok();

            let server_name =
                rustls::pki_types::ServerName::try_from(SNI_DOMAIN)
                    .map_err(|e| format!("无效 SNI '{SNI_DOMAIN}': {e}"))?;
            log_info!("[sni] TLS handshake SNI={SNI_DOMAIN}");
            let tls_stream = tls.connect(server_name, tcp).await
                .map_err(|e| format!("TLS 握手失败 (SNI={SNI_DOMAIN}): {e}"))?;

            Ok(SniConnection(tls_stream))
        })
    }
}

// Always-accept certificate verifier (like C# RemoteCertificateValidationCallback)
#[derive(Debug)]
struct NoVerifier;

impl rustls::client::danger::ServerCertVerifier for NoVerifier {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        rustls::crypto::ring::default_provider()
            .signature_verification_algorithms
            .supported_schemes()
    }
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

pub fn make_url(path: &str) -> String {
    format!("https://{ORIGIN_DOMAIN}{path}")
}

pub fn rewrite_url(url: &str) -> String {
    // rewrite any stale Megumin references back to real domain
    url.replace(SNI_DOMAIN, ORIGIN_DOMAIN)
}

pub fn clear_session_cookies() {
    if let Ok(mut s) = SESSION_COOKIES.write() {
        s.clear();
    }
}

// ---------------------------------------------------------------------------
// DNS resolution (unchanged logic)
// ---------------------------------------------------------------------------

fn load_cached_ip() -> Option<Ipv4Addr> {
    let path = crate::paths::ip_cache_file().ok()?;
    let content = fs::read_to_string(&path).ok()?;
    let ip_str = content.trim();
    ip_str.parse().ok()
}

fn save_ip_cache(ip: Ipv4Addr) {
    let Ok(path) = crate::paths::ip_cache_file() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(&path, ip.to_string());
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

pub async fn init_resolver() {
    refresh_ip_async().await;
    let ip = get_resolved_ip();
    log_info!("[init] {ORIGIN_DOMAIN} -> {ip}");
}

fn is_ip_reachable(ip: Ipv4Addr) -> bool {
    let addr = SocketAddr::new(IpAddr::V4(ip), 443);
    std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3)).is_ok()
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
                .filter(|&ip| is_ip_reachable(ip))
                .or_else(|| resolve_system_dns(ORIGIN_DOMAIN).filter(|&ip| is_ip_reachable(ip)))
                .unwrap_or(Ipv4Addr::new(176, 123, 7, 105));
            save_ip_cache(ip);
            if let Ok(mut c) = CACHED_IP.write() {
                *c = ip;
            }
            ip
        })
}

pub async fn refresh_ip_async() {
    let new_ip = resolve_from_diggui_async()
        .await
        .filter(|&ip| is_ip_reachable(ip))
        .or_else(|| resolve_system_dns(ORIGIN_DOMAIN).filter(|&ip| is_ip_reachable(ip)))
        .unwrap_or(Ipv4Addr::new(176, 123, 7, 105));
    save_ip_cache(new_ip);
    if let Ok(mut c) = CACHED_IP.write() {
        *c = new_ip;
    }
    log_info!("[resolver] {ORIGIN_DOMAIN} -> {new_ip}");
}

async fn resolve_from_diggui_async() -> Option<Ipv4Addr> {
    let client = reqwest::Client::builder()
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
        .await
        .ok()?;

    let body = resp.text().await.ok()?;

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

pub fn force_refresh_ip() {
    let new_ip = resolve_system_dns(ORIGIN_DOMAIN)
        .unwrap_or(Ipv4Addr::new(176, 123, 7, 105));
    log_info!("[resolver] 刷新 IP: {ORIGIN_DOMAIN} -> {new_ip}");
    save_ip_cache(new_ip);
    if let Ok(mut c) = CACHED_IP.write() {
        *c = new_ip;
    }
}

static CACHED_IP: std::sync::LazyLock<RwLock<Ipv4Addr>> =
    std::sync::LazyLock::new(|| RwLock::new(Ipv4Addr::UNSPECIFIED));

// ---------------------------------------------------------------------------
// Challenge / warmup (uses CLIENT which has SNI bypass)
// ---------------------------------------------------------------------------

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

pub async fn get_with_challenge(url: &str) -> Result<Response, String> {
    get_with_challenge_and_account(url, None).await
}

pub async fn get_with_challenge_and_account(
    url: &str,
    account: Option<(&str, &str)>,
) -> Result<Response, String> {
    let cookie = if let Some((uid, ukey)) = account {
        session_cookie_str_with(uid, ukey)
    } else {
        session_cookie_str()
    };

    let resp = CLIENT
        .get(url)
        .header(http::header::COOKIE, &cookie)
        .send()
        .await?;

    let status = resp.status().as_u16();
    if status != 503 {
        return Ok(resp);
    }

    let set_cookie: Vec<String> = resp
        .headers()
        .get_all("set-cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .collect();

    let mut resp = resp;
    let body = resp.text().await?;

    if !body.contains("Checking your browser") {
        return send_request_simple(url, &cookie).await;
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

    let challenge_cookie = crate::solver::build_challenge_cookie(&challenge, solution, elapsed);
    let mut bsrv = String::new();
    for sc in &set_cookie {
        let clean: String = sc.split(';').next().unwrap_or("").to_string();
        if clean.starts_with("bsrv=") {
            bsrv = format!("; {clean}");
        }
    }
    let full_cookie = format!("{challenge_cookie}{bsrv}");

    update_session_from_challenge(&full_cookie, &set_cookie);

    let final_cookie = if let Some((uid, ukey)) = account {
        format!("{full_cookie}; remix_userid={uid}; remix_userkey={ukey}")
    } else {
        full_cookie.clone()
    };

    log_info!("[challenge] cookie: {full_cookie}");

    send_request_simple(url, &final_cookie).await
}

async fn send_request_simple(url: &str, cookie: &str) -> Result<Response, String> {
    match CLIENT
        .get(url)
        .header(http::header::COOKIE, cookie)
        .send()
        .await
    {
        Ok(resp) => Ok(resp),
        Err(e) => {
            log_info!("[retry] 请求失败: {e}，刷新IP重试...");
            force_refresh_ip();
            CLIENT
                .get(url)
                .header(http::header::COOKIE, cookie)
                .send()
                .await
                .map_err(|e2| {
                    let ip = get_resolved_ip();
                    format!(
                        "请求失败 (IP={ip}, SNI={SNI_DOMAIN}, Host={ORIGIN_DOMAIN}): {e2}"
                    )
                })
        }
    }
}

// ---------------------------------------------------------------------------
// Legacy reqwest-based DirectIpResolver (kept for standalone binaries)
// ---------------------------------------------------------------------------

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

impl reqwest::dns::Resolve for DirectIpResolver {
    fn resolve(&self, _name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let addr = SocketAddr::new(IpAddr::V4(self.ip), 0);
        let addrs: reqwest::dns::Addrs = Box::new(std::iter::once(addr));
        Box::pin(std::future::ready(Ok(addrs)))
    }
}
