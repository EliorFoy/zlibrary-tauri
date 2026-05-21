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

#[tokio::main]
async fn main() {
    let ip = std::net::Ipv4Addr::new(176, 123, 7, 105);

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

    let client = reqwest::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .dns_resolver(Arc::new(FixedIpResolver { ip }))
        .default_headers(headers)
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .expect("build");

    let resp = client.get("https://Megumin/s/python").send().await.expect("send");
    let body = resp.text().await.expect("body");
    std::fs::write("challenge_full.html", &body).expect("write");
    println!("Wrote {} bytes to challenge_full.html", body.len());
}