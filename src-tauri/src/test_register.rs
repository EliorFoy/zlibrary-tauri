use std::time::Duration;
use zlibrary_core::mail_receiver::MailReceiver;

fn urlencode(s: &str) -> String {
    s.replace('@', "%40")
}

#[tokio::main]
async fn main() {
    println!("=== Z-Library 注册流程测试 ===\n");

    zlibrary_core::client::init_resolver().await;
    zlibrary_core::logger::init();

    // Step 0: Warmup session (JS challenge)
    println!("[1/5] 预热会话（JS 挑战）...");
    zlibrary_core::client::clear_session_cookies();
    match zlibrary_core::client::warmup_session().await {
        Ok(_) => {
            println!("  ✅ 会话预热完成");
            println!("  🍪 cookies: {}", zlibrary_core::client::registration_cookie_str());
        }
        Err(e) => {
            println!("  ❌ 会话预热失败: {e}");
            return;
        }
    }

    // Step 1: Get temp email
    println!("\n[2/5] 获取临时邮箱...");
    let receiver = zlibrary_core::mail_receiver::MinMailReceiver::new("eliorfoy");
    let email = match receiver.refresh_email().await {
        Ok(e) => {
            println!("  ✅ 邮箱: {e}");
            e
        }
        Err(e) => {
            println!("  ❌ 获取邮箱失败: {e}");
            return;
        }
    };

    // Step 2: Send registration request (send-code)
    println!("\n[3/5] 发送注册请求 (send-code)...");
    let pwd_id: String = rand::random::<u32>().to_string();
    let name_id: String = rand::random::<u32>().to_string();
    let password = format!("ZLib_{}", &pwd_id[..6.min(pwd_id.len())]);
    let name = format!("User_{}", &name_id[..8.min(name_id.len())]);
    println!("  email: {email}");
    println!("  password: {password}");
    println!("  name: {name}");

    let url = zlibrary_core::client::make_url("/papi/user/verification/send-code");
    let cookie = zlibrary_core::client::registration_cookie_str();
    println!("  🍪 cookie: {cookie}");

    let form = reqwest::multipart::Form::new()
        .text("email", email.clone())
        .text("password", password.clone())
        .text("name", name.clone())
        .text("rx", "215".to_string())
        .text("action", "registration".to_string())
        .text("redirectUrl", "".to_string());

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(zlibrary_core::client::ORIGIN_DOMAIN));
    headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
    ));
    headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("*/*"));
    headers.insert(reqwest::header::ACCEPT_LANGUAGE, reqwest::header::HeaderValue::from_static(
        "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6",
    ));
    headers.insert(reqwest::header::ORIGIN, reqwest::header::HeaderValue::from_static("https://z-library.sk"));
    headers.insert(reqwest::header::REFERER, reqwest::header::HeaderValue::from_static("https://z-library.sk/"));
    headers.insert("sec-ch-ua", reqwest::header::HeaderValue::from_static(
        r#""Not:A-Brand";v="99", "Microsoft Edge";v="145", "Chromium";v="145""#,
    ));
    headers.insert("sec-ch-ua-mobile", reqwest::header::HeaderValue::from_static("?0"));
    headers.insert("sec-ch-ua-platform", reqwest::header::HeaderValue::from_static(r#""Windows""#));
    headers.insert("cache-control", reqwest::header::HeaderValue::from_static("no-cache"));
    headers.insert("pragma", reqwest::header::HeaderValue::from_static("no-cache"));
    headers.insert("priority", reqwest::header::HeaderValue::from_static("u=1, i"));
    headers.insert("sec-fetch-dest", reqwest::header::HeaderValue::from_static("empty"));
    headers.insert("sec-fetch-mode", reqwest::header::HeaderValue::from_static("cors"));
    headers.insert("sec-fetch-site", reqwest::header::HeaderValue::from_static("same-origin"));
    headers.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&cookie).unwrap());

    let client = reqwest::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .default_headers(headers)
        .dns_resolver(std::sync::Arc::new(zlibrary_core::client::DirectIpResolver::cached()))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    let resp = client.post(&url).multipart(form).send().await;
    match resp {
        Ok(r) => {
            let status = r.status();
            let set_cookies: Vec<String> = r.headers().get_all("set-cookie")
                .iter()
                .filter_map(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .collect();
            let body = r.text().await.unwrap_or_default();
            println!("  HTTP {status}");
            if !set_cookies.is_empty() {
                println!("  🍪 Set-Cookie: {:?}", set_cookies);
            }
            if body.len() > 500 {
                println!("  Body (前500字): {}", &body[..500]);
            } else {
                println!("  Body: {body}");
            }
            if status.is_success() {
                println!("  ✅ send-code 成功");
            } else if body.contains("Too many registrations") {
                println!("  ❌ 注册频率过高，请稍后再试");
                return;
            } else {
                println!("  ❌ send-code 失败");
                return;
            }
        }
        Err(e) => {
            println!("  ❌ send-code 请求失败: {e}");
            return;
        }
    }

    // Step 3: Load verify modal (browser does this before waiting for code)
    println!("\n[3.5/5] 加载验证码弹窗（模拟浏览器行为）...");
    {
        use zlibrary_core::client;
        let modal_url = client::make_url(&format!(
            "/layer/_modals/verify_action_modal?email={}&action=registration",
            urlencode(&email)
        ));
        let mut modal_headers = reqwest::header::HeaderMap::new();
        modal_headers.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(client::ORIGIN_DOMAIN));
        modal_headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
        ));
        modal_headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("text/html, */*; q=0.01"));
        modal_headers.insert(reqwest::header::ACCEPT_LANGUAGE, reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6"));
        modal_headers.insert(reqwest::header::ORIGIN, reqwest::header::HeaderValue::from_static("https://z-library.sk"));
        modal_headers.insert(reqwest::header::REFERER, reqwest::header::HeaderValue::from_static("https://z-library.sk/"));
        modal_headers.insert("sec-ch-ua", reqwest::header::HeaderValue::from_static(r#""Not:A-Brand";v="99", "Microsoft Edge";v="145", "Chromium";v="145""#));
        modal_headers.insert("sec-ch-ua-mobile", reqwest::header::HeaderValue::from_static("?0"));
        modal_headers.insert("sec-ch-ua-platform", reqwest::header::HeaderValue::from_static(r#""Windows""#));
        modal_headers.insert("sec-fetch-dest", reqwest::header::HeaderValue::from_static("empty"));
        modal_headers.insert("sec-fetch-mode", reqwest::header::HeaderValue::from_static("cors"));
        modal_headers.insert("sec-fetch-site", reqwest::header::HeaderValue::from_static("same-origin"));
        modal_headers.insert("x-requested-with", reqwest::header::HeaderValue::from_static("XMLHttpRequest"));
        modal_headers.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&cookie).unwrap());

        let modal_client = reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(modal_headers)
            .dns_resolver(std::sync::Arc::new(client::DirectIpResolver::cached()))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap();

        match modal_client.get(&modal_url).send().await {
            Ok(r) => {
                let status = r.status();
                let set_cookies: Vec<String> = r.headers().get_all("set-cookie")
                    .iter()
                    .filter_map(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                    .collect();
                println!("  HTTP {status}");
                if !set_cookies.is_empty() {
                    println!("  🍪 Set-Cookie: {:?}", set_cookies);
                }
                if status.is_success() || status.is_redirection() {
                    println!("  ✅ 验证码弹窗加载成功");
                    let body = r.text().await.unwrap_or_default();
                    if body.len() < 2000 {
                        println!("  Body: {body}");
                    } else {
                        println!("  Body (前1000字): {}", &body[..1000]);
                    }
                } else {
                    let body = r.text().await.unwrap_or_default();
                    println!("  ⚠️ 弹窗加载异常: {body}");
                }
            }
            Err(e) => {
                println!("  ⚠️ 弹窗加载请求失败: {e}");
            }
        }
    }

    // Step 4: Wait for verification code
    println!("\n[4/5] 等待验证码邮件（最长 90 秒）...");
    let code = match receiver.wait_for_code("z-lib", 90).await {
        Ok(c) => {
            println!("  ✅ 验证码: {c}");
            c
        }
        Err(e) => {
            println!("  ❌ 等待超时: {e}");
            return;
        }
    };

    // Step 4: Submit verification code
    println!("\n[5/5] 提交验证码...");
    let verify_url = zlibrary_core::client::make_url("/rpc.php");
    let cookie2 = zlibrary_core::client::verify_cookie_str();

    let mut headers2 = reqwest::header::HeaderMap::new();
    headers2.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(zlibrary_core::client::ORIGIN_DOMAIN));
    headers2.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
    ));
    headers2.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static(
        "application/json, text/javascript, */*; q=0.01",
    ));
    headers2.insert(reqwest::header::ACCEPT_LANGUAGE, reqwest::header::HeaderValue::from_static(
        "zh-CN,zh;q=0.9,en;q=0.8,en-GB;q=0.7,en-US;q=0.6",
    ));
    headers2.insert(reqwest::header::ORIGIN, reqwest::header::HeaderValue::from_static("https://z-library.sk"));
    headers2.insert(reqwest::header::REFERER, reqwest::header::HeaderValue::from_static("https://z-library.sk/"));
    headers2.insert("sec-ch-ua", reqwest::header::HeaderValue::from_static(
        r#""Not:A-Brand";v="99", "Microsoft Edge";v="145", "Chromium";v="145""#,
    ));
    headers2.insert("sec-ch-ua-mobile", reqwest::header::HeaderValue::from_static("?0"));
    headers2.insert("sec-ch-ua-platform", reqwest::header::HeaderValue::from_static(r#""Windows""#));
    headers2.insert("sec-fetch-dest", reqwest::header::HeaderValue::from_static("empty"));
    headers2.insert("sec-fetch-mode", reqwest::header::HeaderValue::from_static("cors"));
    headers2.insert("sec-fetch-site", reqwest::header::HeaderValue::from_static("same-origin"));
    headers2.insert("x-requested-with", reqwest::header::HeaderValue::from_static("XMLHttpRequest"));
    headers2.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&cookie2).unwrap());

    let client2 = reqwest::Client::builder()
        .no_proxy()
        .danger_accept_invalid_certs(true)
        .redirect(reqwest::redirect::Policy::none())
        .default_headers(headers2)
        .dns_resolver(std::sync::Arc::new(zlibrary_core::client::DirectIpResolver::cached()))
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    let verify_form: [(&str, &str); 9] = [
        ("isModal", "true"),
        ("email", &email),
        ("password", &password),
        ("name", &name),
        ("rx", "215"),
        ("action", "registration"),
        ("redirectUrl", ""),
        ("verifyCode", &code),
        ("gg_json_mode", "1"),
    ];

    let resp2 = client2.post(&verify_url).form(&verify_form).send().await;
    match resp2 {
        Ok(r) => {
            let status = r.status();
            let set_cookies: Vec<String> = r.headers().get_all("set-cookie")
                .iter()
                .filter_map(|v| v.to_str().ok())
                .map(|s| s.to_string())
                .collect();
            let body = r.text().await.unwrap_or_default();
            println!("  HTTP {status}");
            if !set_cookies.is_empty() {
                println!("  🍪 Set-Cookie: {:?}", set_cookies);
            }
            if body.len() > 600 {
                println!("  Body (前600字): {}", &body[..600]);
            } else {
                println!("  Body: {body}");
            }
            if status.is_success() {
                if let Ok(doc) = serde_json::from_str::<serde_json::Value>(&body) {
                    if let Some(response) = doc.get("response") {
                        if let Some(redirect) = response.get("priorityRedirectUrl").and_then(|v| v.as_str()) {
                            println!("  ✅ 注册成功!");
                            println!("  redirectUrl: {redirect}");
                        } else if let (Some(uid), Some(ukey)) = (
                            response.get("user_id").and_then(|v| v.as_i64()),
                            response.get("user_key").and_then(|v| v.as_str()),
                        ) {
                            println!("  ✅ 注册成功! user_id={uid}, user_key={ukey}");
                        } else {
                            println!("  ⚠️ 状态码成功但无法解析 user_id/user_key");
                            println!("  完整响应: {body}");
                        }
                    } else {
                        println!("  ⚠️ 响应中没有 response 字段");
                        println!("  完整响应: {body}");
                    }
                } else {
                    println!("  ⚠️ 响应不是 JSON");
                    println!("  完整响应: {body}");
                }
            } else {
                println!("  ❌ verify-code 失败");
            }
        }
        Err(e) => {
            println!("  ❌ verify-code 请求失败: {e}");
        }
    }
}
