use std::sync::Arc;
use std::time::Duration;

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

fn build_client(ip: std::net::Ipv4Addr) -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .dns_resolver(Arc::new(FixedIpResolver { ip }))
        .timeout(Duration::from_secs(30))
        .build()
        .expect("build_client")
}

#[tokio::main]
async fn main() {
    println!("=== Z-Library 注册诊断（多IP多策略） ===\n");

    let ips = [
        ("diggui结果", {
            zlibrary_core::client::init_resolver().await;
            zlibrary_core::client::get_resolved_ip()
        }),
        ("硬编码IP", std::net::Ipv4Addr::new(176, 123, 7, 105)),
    ];
    let visitor_ids = ["eliorfoy", "testuser99", "randomvis01"];

    let ua = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36";

    for (ip_label, ip) in &ips {
        println!("\n══════════════════════════════════════");
        println!("  测试IP: {} ({})", ip, ip_label);
        println!("══════════════════════════════════════");

        // 先测试基础连通性
        println!("  [连通性] 测试 /s/python ...");
        let test_resp = build_client(*ip)
            .get("https://Megumin/s/python")
            .header(reqwest::header::HOST, "z-library.sk")
            .header(reqwest::header::USER_AGENT, ua)
            .send()
            .await;
        match test_resp {
            Ok(r) => println!("  [连通性] HTTP {}", r.status()),
            Err(e) => {
                println!("  [连通性] 失败: {}", e);
                continue;
            }
        }

        for vid in &visitor_ids {
            let visitor_id = if *vid == "eliorfoy" {
                vid.to_string()
            } else {
                format!("{}{}", vid, rand::random::<u16>())
            };

            println!("\n  --- visitor_id={} ---", visitor_id);

            // Step 1: 获取邮箱
            let email = match get_temp_email(&visitor_id).await {
                Ok(e) => {
                    println!("  [邮箱] {}", e);
                    e
                }
                Err(e) => {
                    println!("  [邮箱] 失败: {}", e);
                    continue;
                }
            };

            // Step 2: send-code
            let pwd_id: String = rand::random::<u32>().to_string()[..6].to_string();
            let name_id: String = rand::random::<u32>().to_string()[..6].to_string();
            let password = format!("ZLib_{}", pwd_id);
            let name = format!("User_{}", name_id);

            let form = reqwest::multipart::Form::new()
                .text("email", email.clone())
                .text("password", password.clone())
                .text("name", name.clone())
                .text("rx", "215")
                .text("action", "registration")
                .text("redirectUrl", "");

            let client = build_client(*ip);
            let send_result = client
                .post("https://Megumin/papi/user/verification/send-code")
                .header(reqwest::header::HOST, "z-library.sk")
                .header(reqwest::header::USER_AGENT, ua)
                .multipart(form)
                .send()
                .await;

            match send_result {
                Ok(resp) => {
                    let status = resp.status();
                    let body = resp.text().await.unwrap_or_default();
                    let short_body: String = body.chars().take(200).collect();
                    println!("  [send-code] HTTP {} — {}", status, short_body);

                    if body.contains("Too many registrations") {
                        println!("  [结论] 此IP/域名被限，尝试下一个...");
                        break; // 换个IP
                    }
                    if !status.is_success() {
                        println!("  [结论] send-code失败 ({})，尝试下一个visitor_id...", status);
                        continue;
                    }

                    println!("  [OK] send-code成功!");

                    // Step 3: 等验证码
                    println!("  [等待] 验证码...");
                    let code = match wait_for_code("z-lib", &visitor_id, 90).await {
                        Ok(c) => {
                            println!("  [验证码] {}", c);
                            c
                        }
                        Err(e) => {
                            println!("  [验证码] 超时: {}", e);
                            continue;
                        }
                    };

                    // Step 4: 提交验证码
                    println!("  [提交] 验证码...");

                    let form4: [(&str, &str); 9] = [
                        ("isModal", "true"),
                        ("email", &email),
                        ("password", &password),
                        ("name", &name),
                        ("rx", "114"),
                        ("action", "registration"),
                        ("redirectUrl", ""),
                        ("verifyCode", &code),
                        ("gg_json_mode", "1"),
                    ];

                    let verify_result = client
                        .post("https://Megumin/rpc.php")
                        .header(reqwest::header::HOST, "z-library.sk")
                        .header(reqwest::header::USER_AGENT, ua)
                        .header("accept", "application/json, text/javascript, */*; q=0.01")
                        .header("x-requested-with", "XMLHttpRequest")
                        .form(&form4)
                        .send()
                        .await;

                    match verify_result {
                        Ok(resp) => {
                            let status = resp.status();
                            let body = resp.text().await.unwrap_or_default();
                            let short_body: String = body.chars().take(300).collect();
                            println!("  [verify] HTTP {} — {}", status, short_body);

                            if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&body) {
                                if let Some(response) = doc.get("response") {
                                    if let Some(redirect_url) = response
                                        .get("priorityRedirectUrl")
                                        .and_then(|v| v.as_str())
                                    {
                                        println!("  [SUCCESS] 注册成功!");
                                        println!("  redirectUrl: {}", redirect_url);
                                        let re_userid =
                                            regex::Regex::new(r"remix_userid=(\d+)").unwrap();
                                        let re_userkey =
                                            regex::Regex::new(r"remix_userkey=([a-f0-9]+)").unwrap();
                                        if let Some(cap) = re_userid.captures(redirect_url) {
                                            println!("  user_id: {}", &cap[1]);
                                        }
                                        if let Some(cap) = re_userkey.captures(redirect_url) {
                                            println!("  user_key: {}", &cap[1]);
                                        }
                                        println!("\n=== 注册测试成功! ===");
                                        return;
                                    } else if let (Some(uid), Some(ukey)) = (
                                        response.get("user_id").and_then(|v| v.as_i64()),
                                        response.get("user_key").and_then(|v| v.as_str()),
                                    ) {
                                        println!(
                                            "  [SUCCESS] 注册成功! user_id={}, user_key={}",
                                            uid, ukey
                                        );
                                        println!("\n=== 注册测试成功! ===");
                                        return;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            println!("  [verify] 请求失败: {}", e);
                        }
                    }
                }
                Err(e) => {
                    println!("  [send-code] 请求失败: {}", e);
                }
            }
        }
    }

    println!("\n=== 所有策略均失败，请等待一段时间后重试 ===");
}

async fn get_temp_email(visitor_id: &str) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!(
        "https://minmail.app/api/mail/address?refresh=true&expire=1440&part=main&visitor_id={}",
        visitor_id
    );
    let resp = client
        .get(&url)
        .header("visitor-id", visitor_id)
        .send()
        .await
        .map_err(|e| format!("请求失败: {e}"))?;
    let body = resp.text().await.map_err(|e| format!("读取失败: {e}"))?;
    let doc: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {e} — body: {}", body))?;
    doc["address"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("没有 address 字段: {}", body))
}

async fn wait_for_code(
    from_contains: &str,
    visitor_id: &str,
    timeout_secs: u64,
) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;

    let start = std::time::Instant::now();
    let mut attempt = 0;
    while start.elapsed().as_secs() < timeout_secs {
        attempt += 1;
        let url = format!(
            "https://minmail.app/api/mail/list?part=main&visitor_id={}",
            visitor_id
        );
        let resp = client
            .get(&url)
            .header("visitor-id", visitor_id)
            .send()
            .await
            .map_err(|e| format!("请求失败: {e}"))?;
        let body = resp.text().await.map_err(|e| format!("读取失败: {e}"))?;
        let doc: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {e}"))?;

        if let Some(arr) = doc["message"].as_array() {
            for m in arr {
                let from = m["from"].as_str().unwrap_or("");
                if from.contains(from_contains) {
                    let content = m["content"].as_str().unwrap_or("");
                    if let Some(code) = extract_code(content) {
                        return Ok(code);
                    }
                }
            }
        }
        if attempt % 5 == 0 {
            println!(
                "  [WAIT] 已等待 {}s, 第 {} 次检查...",
                start.elapsed().as_secs(),
                attempt
            );
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    Err(format!("{}秒内未收到验证码", timeout_secs))
}

fn extract_code(content: &str) -> Option<String> {
    let re = regex::Regex::new(r"<h1[^>]*>(\d+)</h1>").ok()?;
    if let Some(cap) = re.captures(content) {
        return Some(cap[1].to_string());
    }
    let re2 = regex::Regex::new(r"(\d{4,6})").ok()?;
    if let Some(cap) = re2.captures(content) {
        return Some(cap[1].to_string());
    }
    None
}