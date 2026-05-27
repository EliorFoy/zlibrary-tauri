use std::error::Error;
use std::sync::Arc;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

#[derive(Clone)]
struct FixedIpResolver {
    ip: std::net::Ipv4Addr,
}

impl Resolve for FixedIpResolver {
    fn resolve(&self, _name: Name) -> Resolving {
        let addr = std::net::SocketAddr::new(std::net::IpAddr::V4(self.ip), 0);
        let addrs: Addrs = Box::new(std::iter::once(addr));
        Box::pin(std::future::ready(Ok(addrs)))
    }
}

fn build_test_client(ip: std::net::Ipv4Addr) -> reqwest::Client {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::HOST,
        reqwest::header::HeaderValue::from_static("z-library.sk"),
    );
    headers.insert(
        reqwest::header::USER_AGENT,
        reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
        ),
    );
    headers.insert(
        reqwest::header::COOKIE,
        reqwest::header::HeaderValue::from_static(
            "remix_userkey=a097500143c397d1c09c8c4c459bb142; remix_userid=35246529; selectedSiteMode=books",
        ),
    );
    reqwest::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .dns_resolver(Arc::new(FixedIpResolver { ip }))
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("build_client")
}

#[tokio::main]
async fn main() {
    println!("=== Z-Library 连接诊断 ===\n");

    zlibrary_core::client::init_resolver().await;
    let resolved = zlibrary_core::client::get_resolved_ip();
    let domain = zlibrary_core::client::ORIGIN_DOMAIN;
    println!("[diggui] {domain} -> {resolved}\n");

    let ips = [
        ("diggui.com结果", resolved),
        ("原项目硬编码  ", std::net::Ipv4Addr::new(176, 123, 7, 105)),
    ];

    for (label, ip) in &ips {
        println!("── {label}: {ip} ──");

        println!("  [TCP] {ip}:443");
        match tokio::time::timeout(
            std::time::Duration::from_secs(5),
            tokio::net::TcpStream::connect(format!("{ip}:443")),
        )
        .await
        {
            Ok(Ok(_)) => println!("    ✓ TCP OK"),
            Ok(Err(e)) => { println!("    ✗ TCP: {e}"); continue; }
            Err(_) => { println!("    ✗ 超时"); continue; }
        }

        let client = build_test_client(*ip);
        let url = "https://Megumin/s/python";
        println!("  [HTTPS] GET {url}");
        match client.get(url).send().await {
            Ok(resp) => {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                println!("    HTTP {status} — {} bytes", body.len());
                let preview: String = body.chars().take(200).collect();
                println!("    {}", preview.replace('\n', " "));
            }
            Err(e) => {
                println!("    ✗ {e}");
            }
        }
    }

    println!("\n=== 完成 ===");
}