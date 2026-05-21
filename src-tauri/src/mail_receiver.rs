use std::time::Duration;

use regex::Regex;
use serde::Deserialize;

#[async_trait::async_trait]
pub trait MailReceiver: Send + Sync {
    async fn refresh_email(&self) -> Result<String, String>;
    async fn get_emails(&self) -> Result<Vec<MailMessage>, String>;
    async fn wait_for_code(&self, from_contains: &str, timeout_secs: u64) -> Result<String, String>;
}

#[derive(Debug, Clone, Deserialize)]
pub struct MailMessage {
    pub from: String,
    pub subject: String,
    pub content: Option<String>,
}

pub struct MinMailReceiver {
    client: reqwest::Client,
    visitor_id: String,
}

impl MinMailReceiver {
    pub fn new(visitor_id: &str) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert("accept", reqwest::header::HeaderValue::from_static("*/*"));
        headers.insert(
            "accept-language",
            reqwest::header::HeaderValue::from_static("zh-CN,zh;q=0.9,en;q=0.8"),
        );
        headers.insert("cache-control", reqwest::header::HeaderValue::from_static("no-cache"));
        headers.insert("pragma", reqwest::header::HeaderValue::from_static("no-cache"));
        headers.insert("priority", reqwest::header::HeaderValue::from_static("u=1, i"));
        headers.insert(
            "referer",
            reqwest::header::HeaderValue::from_static("https://minmail.app/cn"),
        );
        headers.insert(
            "sec-ch-ua",
            reqwest::header::HeaderValue::from_static(
                r#""Microsoft Edge";v="137", "Chromium";v="137", "Not/A)Brand";v="24""#,
            ),
        );
        headers.insert("sec-ch-ua-mobile", reqwest::header::HeaderValue::from_static("?0"));
        headers.insert(
            "sec-ch-ua-platform",
            reqwest::header::HeaderValue::from_static(r#""Windows""#),
        );
        headers.insert("sec-fetch-dest", reqwest::header::HeaderValue::from_static("empty"));
        headers.insert("sec-fetch-mode", reqwest::header::HeaderValue::from_static("cors"));
        headers.insert("sec-fetch-site", reqwest::header::HeaderValue::from_static("same-origin"));
        headers.insert(
            "user-agent",
            reqwest::header::HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/137.0.0.0 Safari/537.36 Edg/137.0.0.0",
            ),
        );
        headers.insert(
            "visitor-id",
            reqwest::header::HeaderValue::from_str(visitor_id).unwrap(),
        );

        let client = reqwest::Client::builder()
            .no_proxy()
            .default_headers(headers)
            .timeout(Duration::from_secs(15))
            .build()
            .expect("MinMailReceiver client");

        Self {
            client,
            visitor_id: visitor_id.to_string(),
        }
    }
}

#[async_trait::async_trait]
impl MailReceiver for MinMailReceiver {
    async fn refresh_email(&self) -> Result<String, String> {
        let url = format!(
            "https://minmail.app/api/mail/address?refresh=true&expire=1440&part=main&visitor_id={}",
            self.visitor_id
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("刷新邮箱失败: {e}"))?;
        let body = resp
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;
        let doc: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {e}"))?;
        doc["address"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "响应中没有邮箱地址".to_string())
    }

    async fn get_emails(&self) -> Result<Vec<MailMessage>, String> {
        let url = format!(
            "https://minmail.app/api/mail/list?part=main&visitor_id={}",
            self.visitor_id
        );
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("获取邮件列表失败: {e}"))?;
        let body = resp
            .text()
            .await
            .map_err(|e| format!("读取响应失败: {e}"))?;
        let doc: serde_json::Value =
            serde_json::from_str(&body).map_err(|e| format!("JSON 解析失败: {e}"))?;

        let mut messages = Vec::new();
        if let Some(arr) = doc["message"].as_array() {
            for m in arr {
                messages.push(MailMessage {
                    from: m["from"].as_str().unwrap_or("").to_string(),
                    subject: m["subject"].as_str().unwrap_or("").to_string(),
                    content: m["content"].as_str().map(|s| s.to_string()),
                });
            }
        }
        Ok(messages)
    }

    async fn wait_for_code(&self, from_contains: &str, timeout_secs: u64) -> Result<String, String> {
        let start = std::time::Instant::now();
        while start.elapsed().as_secs() < timeout_secs {
            let emails = self.get_emails().await?;
            for email in &emails {
                if email.from.contains(from_contains) {
                    if let Some(code) = extract_verification_code(email.content.as_deref()) {
                        return Ok(code);
                    }
                }
            }
            tokio::time::sleep(Duration::from_secs(3)).await;
        }
        Err(format!("{timeout_secs}秒内未收到验证码"))
    }
}

fn extract_verification_code(content: Option<&str>) -> Option<String> {
    let content = content?;
    let re = Regex::new(r"<h1[^>]*>(\d+)</h1>").ok()?;
    if let Some(cap) = re.captures(content) {
        return Some(cap[1].to_string());
    }
    let re2 = Regex::new(r"(\d{4,6})").ok()?;
    if let Some(cap) = re2.captures(content) {
        return Some(cap[1].to_string());
    }
    None
}