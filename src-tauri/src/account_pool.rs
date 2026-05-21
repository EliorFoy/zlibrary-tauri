use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use rand::Rng;
use rusqlite::Connection;

use crate::log_info;
use crate::mail_receiver::MailReceiver;

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
    db: Connection,
    mail_receiver: Option<Arc<dyn MailReceiver>>,
}

impl AccountPool {
    pub fn new() -> Result<Self, String> {
        let path = db_path();
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
            db,
            mail_receiver: None,
        })
    }

    pub fn with_mail_receiver(mut self, receiver: Arc<dyn MailReceiver>) -> Self {
        self.mail_receiver = Some(receiver);
        self
    }

    pub fn get_random_account(&self) -> Option<AccountInfo> {
        let mut stmt = self
            .db
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

        let now = now_str();
        let new_count = if account.usage_count > 0 {
            account.usage_count - 1
        } else {
            9
        };

        self.db
            .execute(
                "UPDATE accounts SET last_used = ?1, usage_count = ?2 WHERE id = ?3",
                rusqlite::params![now, new_count, account.id],
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
        let email = receiver.refresh_email().await?;
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

        self.send_registration_request(&email, &password, &name)
            .await?;

        let code = receiver.wait_for_code("z-lib", 60).await?;

        self.submit_verification_code(&email, &password, &name, &code)
            .await?;

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

        let client = reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .dns_resolver(std::sync::Arc::new(client::DirectIpResolver::cached()))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(&url)
            .header(reqwest::header::HOST, client::ORIGIN_DOMAIN)
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
            )
            .multipart(form)
            .send()
            .await
            .map_err(|e| format!("send-code 请求失败: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
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
            ("rx", "114"),
            ("action", "registration"),
            ("redirectUrl", ""),
            ("verifyCode", code),
            ("gg_json_mode", "1"),
        ];

        let client = reqwest::Client::builder()
            .no_proxy()
            .danger_accept_invalid_certs(true)
            .dns_resolver(std::sync::Arc::new(client::DirectIpResolver::cached()))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;

        let resp = client
            .post(&url)
            .header(reqwest::header::HOST, client::ORIGIN_DOMAIN)
            .header(
                reqwest::header::USER_AGENT,
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36",
            )
            .header("accept", "application/json, text/javascript, */*; q=0.01")
            .header("x-requested-with", "XMLHttpRequest")
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("verify-code 请求失败: {e}"))?;

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

        self.db
            .execute(
                "INSERT OR IGNORE INTO accounts (email, password, username, user_id, user_key, registration_date, usage_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 10)",
                rusqlite::params![email, password, name, user_id, user_key, now_str()],
            )
            .map_err(|e| format!("保存账号失败: {e}"))?;

        Ok(())
    }

    pub fn list_accounts(&self) -> Result<Vec<AccountInfo>, String> {
        let mut stmt = self
            .db
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
        self.db
            .execute("DELETE FROM accounts WHERE id = ?1", rusqlite::params![id])
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn db_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
        .join("zlibrary_accounts.db")
}

fn now_str() -> String {
    std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", "Get-Date -Format 'yyyy-MM-dd HH:mm:ss'"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() >= 19)
        .unwrap_or_else(|| "2025-01-01 00:00:00".to_string())
}