use std::sync::{Arc, Mutex};
use std::time::Duration;

use rand::Rng;
use rusqlite::Connection;

use crate::log_info;
use crate::mail_receiver::MailReceiver;

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct AccountInfo {
    pub id: i64,
    pub email: String,
    pub user_id: i64,
    pub user_key: String,
    pub usage_count: i32,
}

pub struct RegistrationResult {
    pub total: u32,
    pub success: u32,
    pub fail: u32,
}

pub struct AccountPool {
    db: Mutex<Connection>,
    mail_receiver: Option<Arc<dyn MailReceiver>>,
}

impl AccountPool {
    fn build_registration_client(&self, extra_headers: reqwest::header::HeaderMap) -> Result<reqwest::Client, String> {
        use crate::client;
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(client::ORIGIN_DOMAIN));
        headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
        ));
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
        for (k, v) in extra_headers.iter() {
            headers.insert(k, v.clone());
        }
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&client::registration_cookie_str()).map_err(|e| e.to_string())?,
        );

        reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .dns_resolver(std::sync::Arc::new(client::DirectIpResolver::cached()))
            .cookie_store(true)
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())
    }

    pub fn new() -> Result<Self, String> {
        let path = db_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("创建目录失败: {e}"))?;
        }
        let db = Connection::open(&path).map_err(|e| format!("打开数据库失败: {e}"))?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS accounts (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                email TEXT NOT NULL UNIQUE,
                password TEXT NOT NULL,
                username TEXT NOT NULL,
                user_id INTEGER NOT NULL,
                user_key TEXT NOT NULL,
                registration_date TEXT NOT NULL,
                last_used TEXT,
                usage_count INTEGER NOT NULL DEFAULT 10
            )",
        )
        .map_err(|e| format!("建表失败: {e}"))?;
        Ok(Self {
            db: Mutex::new(db),
            mail_receiver: None,
        })
    }

    pub fn with_mail_receiver(mut self, receiver: Arc<dyn MailReceiver>) -> Self {
        self.mail_receiver = Some(receiver);
        self
    }

    pub fn get_random_account(&self) -> Option<AccountInfo> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare(
                "SELECT id, email, user_id, user_key, usage_count
                 FROM accounts
                 WHERE usage_count > 0
                    OR last_used IS NULL
                    OR datetime(last_used) < datetime('now', '-1 day')
                 ORDER BY
                    CASE
                        WHEN datetime(last_used) < datetime('now', '-1 day') OR last_used IS NULL THEN 0
                        ELSE 1
                    END,
                    last_used ASC
                 LIMIT 1",
            )
            .ok()?;

        let account = stmt
            .query_row([], |row| {
                Ok(AccountInfo {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    user_id: row.get(2)?,
                    user_key: row.get(3)?,
                    usage_count: row.get(4)?,
                })
            })
            .ok()?;

        let new_count = if account.usage_count > 0 {
            account.usage_count - 1
        } else {
            9
        };

        self.db.lock().unwrap()
            .execute(
                "UPDATE accounts SET last_used = datetime('now'), usage_count = ?1 WHERE id = ?2",
                rusqlite::params![new_count, account.id],
            )
            .ok()?;

        Some(account)
    }

    pub async fn auto_register(
        &self,
        count: u32,
        mail_receiver: &dyn MailReceiver,
    ) -> RegistrationResult {
        let mut result = RegistrationResult {
            total: count,
            success: 0,
            fail: 0,
        };

        for i in 0..count {
            match self.register_single(mail_receiver).await {
                Ok(_) => result.success += 1,
                Err(e) => {
                    log_info!("[account] 注册失败 ({}/{}): {e}", i + 1, count);
                    result.fail += 1;
                }
            }
            if i + 1 < count {
                let delay = rand::thread_rng().gen_range(3000..8000);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
        }

        result
    }

    async fn register_single(&self, receiver: &dyn MailReceiver) -> Result<(), String> {
        let email = receiver.refresh_email().await.map_err(|e| {
            #[cfg(debug_assertions)] eprintln!("[DEBUG] refresh_email 失败: {e}");
            e
        })?;
        #[cfg(debug_assertions)] eprintln!("[DEBUG] 临时邮箱: {email}");

        let pwd_id: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(6)
            .map(char::from)
            .collect();
        let name_id: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect();
        let password = format!("ZLib_{}", pwd_id);
        let name = format!("User_{}", name_id);

        let client = self.build_registration_client(reqwest::header::HeaderMap::new())?;

        self.send_registration_request_with_client(&email, &password, &name, &client)
            .await
            .map_err(|e| {
                #[cfg(debug_assertions)] eprintln!("[DEBUG] send-code 失败: {e}");
                e
            })?;
        #[cfg(debug_assertions)] eprintln!("[DEBUG] send-code 成功");

        if let Err(_e) = self.load_verify_modal_with_client(&email, &client).await {
            #[cfg(debug_assertions)] eprintln!("[DEBUG] load_verify_modal 失败: {_e}");
        }

        let code = receiver.wait_for_code("z-lib", 60).await.map_err(|e| {
            #[cfg(debug_assertions)] eprintln!("[DEBUG] wait_for_code 失败: {e}");
            e
        })?;
        #[cfg(debug_assertions)] eprintln!("[DEBUG] 验证码: {code}");

        self.submit_verification_code_with_client(&email, &password, &name, &code, &client)
            .await
            .map_err(|e| {
                #[cfg(debug_assertions)] eprintln!("[DEBUG] submit_verification_code 失败: {e}");
                e
            })?;
        #[cfg(debug_assertions)] eprintln!("[DEBUG] 注册成功!");

        Ok(())
    }

    async fn send_registration_request_with_client(
        &self,
        email: &str,
        password: &str,
        name: &str,
        client: &reqwest::Client,
    ) -> Result<(), String> {
        use crate::client;
        let url = client::make_url("/papi/user/verification/send-code");

        let form = reqwest::multipart::Form::new()
            .text("email", email.to_string())
            .text("password", password.to_string())
            .text("name", name.to_string())
            .text("rx", "215".to_string())
            .text("action", "registration".to_string())
            .text("redirectUrl", "".to_string());

        let mut extra = reqwest::header::HeaderMap::new();
        extra.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("*/*"));

        let resp = client
            .post(&url)
            .headers(extra)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("send-code 请求失败: {e}"))?;

        let status = resp.status();
        #[cfg(debug_assertions)]
        eprintln!("[DEBUG] send-code HTTP {status}");

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            #[cfg(debug_assertions)]
            eprintln!("[DEBUG] send-code body (前300字): {}", &body.chars().take(300).collect::<String>());
            if body.contains("Too many registrations") {
                return Err("注册频率过高，请稍后再试".into());
            }
            return Err(format!("发送验证码失败 HTTP {status}: {body}"));
        }

        Ok(())
    }

    async fn load_verify_modal_with_client(
        &self,
        email: &str,
        client: &reqwest::Client,
    ) -> Result<(), String> {
        use crate::client;
        let encoded_email: String = email.chars().map(|c| match c {
            '@' => "%40".to_string(),
            _ => c.to_string(),
        }).collect();
        let url = client::make_url(&format!(
            "/layer/_modals/verify_action_modal?email={}&action=registration",
            encoded_email
        ));

        let mut extra = reqwest::header::HeaderMap::new();
        extra.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("text/html, */*; q=0.01"));
        extra.insert("x-requested-with", reqwest::header::HeaderValue::from_static("XMLHttpRequest"));

        let resp = client
            .get(&url)
            .headers(extra)
            .send()
            .await
            .map_err(|e| format!("验证码弹窗请求失败: {e}"))?;

        let status = resp.status();
        #[cfg(debug_assertions)]
        eprintln!("[DEBUG] verify-modal HTTP {status}");

        if !status.is_success() && !status.is_redirection() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("验证码弹窗加载失败 HTTP {status}: {body}"));
        }

        Ok(())
    }

    async fn submit_verification_code_with_client(
        &self,
        email: &str,
        password: &str,
        name: &str,
        code: &str,
        client: &reqwest::Client,
    ) -> Result<(), String> {
        use crate::client;
        let url = client::make_url("/rpc.php");

        let form = [
            ("isModal", "true"),
            ("email", email),
            ("password", password),
            ("name", name),
            ("rx", "215"),
            ("action", "registration"),
            ("redirectUrl", ""),
            ("verifyCode", code),
            ("gg_json_mode", "1"),
        ];

        let mut extra = reqwest::header::HeaderMap::new();
        extra.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static(
            "application/json, text/javascript, */*; q=0.01",
        ));
        extra.insert("x-requested-with", reqwest::header::HeaderValue::from_static("XMLHttpRequest"));
        extra.insert(
            reqwest::header::CONTENT_TYPE,
            reqwest::header::HeaderValue::from_static("application/x-www-form-urlencoded; charset=UTF-8"),
        );

        let resp = client
            .post(&url)
            .headers(extra)
            .header(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&crate::client::verify_cookie_str()).map_err(|e| e.to_string())?)
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("verify-code 请求失败: {e}"))?;

        let status = resp.status();
        #[cfg(debug_assertions)]
        eprintln!("[DEBUG] verify-code HTTP {status}");

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            #[cfg(debug_assertions)]
            eprintln!("[DEBUG] verify-code body: {}", &body.chars().take(200).collect::<String>());
            return Err(format!("验证码提交失败 HTTP {status}: {body}"));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;

        let doc: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {e}"))?;

        let response = doc
            .get("response")
            .ok_or_else(|| format!("响应格式错误: {body}"))?;

        let user_id: i64;
        let user_key: String;

        if let Some(redirect_url) = response.get("priorityRedirectUrl").and_then(|v| v.as_str()) {
            let re_userid = regex::Regex::new(r"remix_userid=(\d+)").unwrap();
            let re_userkey = regex::Regex::new(r"remix_userkey=([a-f0-9]+)").unwrap();
            user_id = re_userid
                .captures(redirect_url)
                .and_then(|c| c[1].parse().ok())
                .ok_or("无法提取 user_id")?;
            user_key = re_userkey
                .captures(redirect_url)
                .and_then(|c| Some(c[1].to_string()))
                .ok_or("无法提取 user_key")?;
        } else {
            user_id = response
                .get("user_id")
                .and_then(|v| v.as_i64())
                .ok_or("响应中没有 user_id")?;
            user_key = response
                .get("user_key")
                .and_then(|v| v.as_str())
                .ok_or("响应中没有 user_key")?
                .to_string();
        }

        self.db.lock().unwrap()
            .execute(
                "INSERT OR IGNORE INTO accounts (email, password, username, user_id, user_key, registration_date, usage_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), 10)",
                rusqlite::params![email, password, name, user_id, user_key],
            )
            .map_err(|e| format!("保存账号失败: {e}"))?;

        Ok(())
    }

    pub async fn manual_register(
        &self,
        email: &str,
        password: &str,
        name: &str,
        code: &str,
    ) -> Result<(), String> {
        self.submit_verification_code(email, password, name, code)
            .await
    }

    pub async fn manual_login(&self, email: &str, password: &str) -> Result<(), String> {
        use crate::client;
        let url = client::make_url("/rpc.php");

        let form = [
            ("isModal", "true"),
            ("email", email),
            ("password", password),
            ("site_mode", "books"),
            ("action", "login"),
            ("redirectUrl", "https://z-library.sk/"),
            ("gg_json_mode", "1"),
        ];

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(client::ORIGIN_DOMAIN));
        headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
        ));
        headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static(
            "application/json, text/javascript, */*; q=0.01",
        ));
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
        headers.insert("x-requested-with", reqwest::header::HeaderValue::from_static("XMLHttpRequest"));
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&client::registration_cookie_str()).map_err(|e| e.to_string())?,
        );

        let cl = reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .dns_resolver(std::sync::Arc::new(client::DirectIpResolver::cached()))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = cl
            .post(&url)
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("登录请求失败: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("登录失败 HTTP {status}: {body}"));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;

        let doc: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {e}"))?;

        let response = doc
            .get("response")
            .ok_or_else(|| format!("响应格式错误: {body}"))?;

        if response.get("validationError").and_then(|v| v.as_bool()).unwrap_or(false) {
            let msg = response.get("message").and_then(|v| v.as_str()).unwrap_or("未知错误");
            return Err(format!("登录失败: {msg}"));
        }

        let user_id = response
            .get("user_id")
            .and_then(|v| v.as_i64())
            .ok_or("响应中没有 user_id")?;
        let user_key = response
            .get("user_key")
            .and_then(|v| v.as_str())
            .ok_or("响应中没有 user_key")?
            .to_string();
        let name = response
            .get("user_name")
            .and_then(|v| v.as_str())
            .unwrap_or(email)
            .to_string();

        self.db.lock().unwrap()
            .execute(
                "INSERT OR IGNORE INTO accounts (email, password, username, user_id, user_key, registration_date, usage_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), 10)",
                rusqlite::params![email, password, name, user_id, user_key],
            )
            .map_err(|e| format!("保存账号失败: {e}"))?;

        Ok(())
    }

    pub async fn load_verify_modal(&self, email: &str) -> Result<(), String> {
        use crate::client;
        let encoded_email: String = email.chars().map(|c| match c {
            '@' => "%40".to_string(),
            _ => c.to_string(),
        }).collect();
        let url = client::make_url(&format!(
            "/layer/_modals/verify_action_modal?email={}&action=registration",
            encoded_email
        ));

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(client::ORIGIN_DOMAIN));
        headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
        ));
        headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("text/html, */*; q=0.01"));
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
        headers.insert("sec-fetch-dest", reqwest::header::HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", reqwest::header::HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", reqwest::header::HeaderValue::from_static("same-origin"));
        headers.insert("x-requested-with", reqwest::header::HeaderValue::from_static("XMLHttpRequest"));
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&client::registration_cookie_str()).map_err(|e| e.to_string())?,
        );

        let cl = reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .dns_resolver(std::sync::Arc::new(client::DirectIpResolver::cached()))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = cl
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("验证码弹窗请求失败: {e}"))?;

        let status = resp.status();
        #[cfg(debug_assertions)]
        eprintln!("[DEBUG] verify-modal HTTP {status}");

        if !status.is_success() && !status.is_redirection() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("验证码弹窗加载失败 HTTP {status}: {body}"));
        }

        Ok(())
    }

    pub async fn send_code_for_email(
        &self,
        email: &str,
        password: &str,
        name: &str,
    ) -> Result<(), String> {
        self.send_registration_request(email, password, name).await
    }

    async fn send_registration_request(
        &self,
        email: &str,
        password: &str,
        name: &str,
    ) -> Result<(), String> {
        use crate::client;
        let url = client::make_url("/papi/user/verification/send-code");

        let form = reqwest::multipart::Form::new()
            .text("email", email.to_string())
            .text("password", password.to_string())
            .text("name", name.to_string())
            .text("rx", "215".to_string())
            .text("action", "registration".to_string())
            .text("redirectUrl", "".to_string());

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(client::ORIGIN_DOMAIN));
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
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&client::registration_cookie_str()).map_err(|e| e.to_string())?,
        );

        let client = reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .dns_resolver(std::sync::Arc::new(client::DirectIpResolver::cached()))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("send-code 请求失败: {e}"))?;

        let status = resp.status();
        #[cfg(debug_assertions)]
        eprintln!("[DEBUG] send-code HTTP {status}");

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            #[cfg(debug_assertions)]
            eprintln!("[DEBUG] send-code body (前300字): {}", &body.chars().take(300).collect::<String>());
            if body.contains("Too many registrations") {
                return Err("注册频率过高，请稍后再试".into());
            }
            return Err(format!("发送验证码失败 HTTP {status}: {body}"));
        }

        Ok(())
    }

    async fn submit_verification_code(
        &self,
        email: &str,
        password: &str,
        name: &str,
        code: &str,
    ) -> Result<(), String> {
        use crate::client;
        let url = client::make_url("/rpc.php");

        let form = [
            ("isModal", "true"),
            ("email", email),
            ("password", password),
            ("name", name),
            ("rx", "215"),
            ("action", "registration"),
            ("redirectUrl", ""),
            ("verifyCode", code),
            ("gg_json_mode", "1"),
        ];

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(client::ORIGIN_DOMAIN));
        headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
        ));
        headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static(
            "application/json, text/javascript, */*; q=0.01",
        ));
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
        headers.insert("x-requested-with", reqwest::header::HeaderValue::from_static("XMLHttpRequest"));
        headers.insert(
            reqwest::header::COOKIE,
            reqwest::header::HeaderValue::from_str(&client::verify_cookie_str()).map_err(|e| e.to_string())?,
        );

        let client = reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .redirect(reqwest::redirect::Policy::none())
            .default_headers(headers)
            .dns_resolver(std::sync::Arc::new(client::DirectIpResolver::cached()))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(&url)
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("verify-code 请求失败: {e}"))?;

        let status = resp.status();
        #[cfg(debug_assertions)]
        eprintln!("[DEBUG] verify-code HTTP {status}");

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            #[cfg(debug_assertions)]
            eprintln!("[DEBUG] verify-code body: {}", &body.chars().take(200).collect::<String>());
            return Err(format!("验证码提交失败 HTTP {status}: {body}"));
        }

        let body = resp
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;

        let doc: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {e}"))?;

        let response = doc
            .get("response")
            .ok_or_else(|| format!("响应格式错误: {body}"))?;

        let user_id: i64;
        let user_key: String;

        if let Some(redirect_url) = response.get("priorityRedirectUrl").and_then(|v| v.as_str()) {
            let re_userid = regex::Regex::new(r"remix_userid=(\d+)").unwrap();
            let re_userkey = regex::Regex::new(r"remix_userkey=([a-f0-9]+)").unwrap();
            user_id = re_userid
                .captures(redirect_url)
                .and_then(|c| c[1].parse().ok())
                .ok_or("无法提取 user_id")?;
            user_key = re_userkey
                .captures(redirect_url)
                .and_then(|c| Some(c[1].to_string()))
                .ok_or("无法提取 user_key")?;
        } else {
            user_id = response
                .get("user_id")
                .and_then(|v| v.as_i64())
                .ok_or("响应中没有 user_id")?;
            user_key = response
                .get("user_key")
                .and_then(|v| v.as_str())
                .ok_or("响应中没有 user_key")?
                .to_string();
        }

        self.db.lock().unwrap()
            .execute(
                "INSERT OR IGNORE INTO accounts (email, password, username, user_id, user_key, registration_date, usage_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'), 10)",
                rusqlite::params![email, password, name, user_id, user_key],
            )
            .map_err(|e| format!("保存账号失败: {e}"))?;

        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountInfo>, String> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare(
                "SELECT id, email, user_id, user_key, usage_count FROM accounts ORDER BY id",
            )
            .map_err(|e| e.to_string())?;

        let accounts = stmt
            .query_map([], |row| {
                Ok(AccountInfo {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    user_id: row.get(2)?,
                    user_key: row.get(3)?,
                    usage_count: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(accounts)
    }

    pub fn delete_account(&self, id: i64) -> Result<(), String> {
        self.db.lock().unwrap()
            .execute("DELETE FROM accounts WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_active_account(&self, id: i64) -> Result<(), String> {
        self.db.lock().unwrap()
            .execute(
                "INSERT OR REPLACE INTO settings (key, value) VALUES ('active_account_id', ?1)",
                rusqlite::params![id.to_string()],
            )
            .map_err(|e| format!("设置活跃账号失败: {e}"))?;
        Ok(())
    }

    pub fn get_active_account_id(&self) -> Option<i64> {
        let db = self.db.lock().unwrap();
        // Ensure settings table exists
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )"
        ).ok();
        
        db.query_row(
            "SELECT value FROM settings WHERE key = 'active_account_id'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
    }

    pub fn get_active_account(&self) -> Option<AccountInfo> {
        let active_id = self.get_active_account_id()?;
        let db = self.db.lock().unwrap();
        db.query_row(
            "SELECT id, email, user_id, user_key, usage_count FROM accounts WHERE id = ?1",
            rusqlite::params![active_id],
            |row| Ok(AccountInfo {
                id: row.get(0)?,
                email: row.get(1)?,
                user_id: row.get(2)?,
                user_key: row.get(3)?,
                usage_count: row.get(4)?,
            }),
        )
        .ok()
    }

    pub fn decrement_usage(&self, id: i64) -> Result<(), String> {
        self.db.lock().unwrap()
            .execute(
                "UPDATE accounts SET usage_count = MAX(usage_count - 1, 0) WHERE id = ?1",
                rusqlite::params![id],
            )
            .map_err(|e| format!("扣减额度失败: {e}"))?;
        Ok(())
    }

    pub fn has_any_available_account(&self) -> bool {
        let db = self.db.lock().unwrap();
        db.query_row(
            "SELECT COUNT(*) FROM accounts WHERE usage_count > 0",
            [],
            |row| row.get::<_, i32>(0),
        )
        .map(|c| c > 0)
        .unwrap_or(false)
    }

    pub fn get_best_available_account(&self) -> Option<AccountInfo> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare(
                "SELECT id, email, user_id, user_key, usage_count
                 FROM accounts
                 WHERE usage_count > 0
                 ORDER BY usage_count DESC, last_used ASC
                 LIMIT 1",
            )
            .ok()?;

        stmt.query_row([], |row| {
            Ok(AccountInfo {
                id: row.get(0)?,
                email: row.get(1)?,
                user_id: row.get(2)?,
                user_key: row.get(3)?,
                usage_count: row.get(4)?,
            })
        })
        .ok()
    }

    pub fn get_account_summary(&self) -> Result<Vec<AccountInfo>, String> {
        let db = self.db.lock().unwrap();
        let mut stmt = db
            .prepare(
                "SELECT id, email, user_id, user_key, usage_count FROM accounts ORDER BY id",
            )
            .map_err(|e| e.to_string())?;

        let accounts = stmt
            .query_map([], |row| {
                Ok(AccountInfo {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    user_id: row.get(2)?,
                    user_key: row.get(3)?,
                    usage_count: row.get(4)?,
                })
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();

        Ok(accounts)
    }

    pub async fn refresh_account_quota(&self, id: i64) -> Result<i32, String> {
        use crate::client;
        let account = {
            let db = self.db.lock().unwrap();
            db.query_row(
                "SELECT id, email, user_id, user_key FROM accounts WHERE id = ?1",
                rusqlite::params![id],
                |row| Ok(AccountInfo {
                    id: row.get(0)?,
                    email: row.get(1)?,
                    user_id: row.get(2)?,
                    user_key: row.get(3)?,
                    usage_count: 0,
                }),
            ).map_err(|e| format!("查询账号失败: {e}"))?
        };

        let uid_str = account.user_id.to_string();
        let cookie = client::session_cookie_str_with(&uid_str, &account.user_key);
        let url = client::make_url("/");

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(reqwest::header::HOST, reqwest::header::HeaderValue::from_static(client::ORIGIN_DOMAIN));
        headers.insert(reqwest::header::USER_AGENT, reqwest::header::HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/145.0.0.0 Safari/537.36 Edg/145.0.0.0",
        ));
        headers.insert(reqwest::header::ACCEPT, reqwest::header::HeaderValue::from_static("text/html,*/*;q=0.01"));
        headers.insert(reqwest::header::ACCEPT_LANGUAGE, reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"));
        headers.insert(reqwest::header::COOKIE, reqwest::header::HeaderValue::from_str(&cookie).map_err(|e| e.to_string())?);

        let mut resp = client::get_with_challenge_and_account(&url, Some((&uid_str, &account.user_key))).await?;

        let html = resp.text().await.map_err(|e| format!("读取页面失败: {e}"))?;

        let remaining = parse_remaining_downloads(&html);
        let new_count = remaining.unwrap_or(-1);

        self.db.lock().unwrap()
            .execute(
                "UPDATE accounts SET usage_count = ?1 WHERE id = ?2",
                rusqlite::params![new_count, id],
            )
            .map_err(|e| e.to_string())?;

        Ok(new_count)
    }

    pub async fn refresh_all_quotas(&self) -> Result<Vec<(i64, String, i32)>, String> {
        let accounts = self.list_accounts()?;
        let mut results = Vec::new();

        for acct in &accounts {
            let count = self.refresh_account_quota(acct.id).await.unwrap_or(-1);
            results.push((acct.id, acct.email.clone(), count));
        }

        Ok(results)
    }
}

fn db_path() -> Result<std::path::PathBuf, String> {
    crate::paths::account_db_path()
}

fn parse_remaining_downloads(html: &str) -> Option<i32> {
    let re = regex::Regex::new(
        r#"(?s)caret-scroll__title[^>]*>\s*(\d+)/(\d+)\s*</div>\s*<div[^>]*class="caret-scroll__desc"[^>]*>.*?<span>\s*Daily limit\s*</span>"#
    ).ok()?;
    if let Some(cap) = re.captures(html) {
        let used: i32 = cap[1].parse().ok()?;
        let total: i32 = cap[2].parse().ok()?;
        return Some(total - used);
    }

    let re = regex::Regex::new(
        r#"(?s)<div class="caret-scroll__title">\s*(\d+)/(\d+)\s*</div>"#
    ).ok()?;
    if let Some(cap) = re.captures(html) {
        let used: i32 = cap[1].parse().ok()?;
        let total: i32 = cap[2].parse().ok()?;
        return Some(total - used);
    }

    None
}
