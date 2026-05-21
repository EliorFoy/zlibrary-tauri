use crate::client;
use crate::model::{BookInfo, SearchResult};

pub async fn search_books(query: &str, page: u32) -> Result<SearchResult, String> {
    let encoded = urlencoding(query);
    let url = client::make_url(&format!("/s/{}?page={}", encoded, page));

    let resp = client::get_with_challenge(&url).await?;
    let html = resp.text().await.map_err(|e| format!("读取失败: {e}"))?;

    let _ = std::fs::write("debug_response.html", &html);

    let books = parse_books(&html);
    let total = books.len() as u32;

    Ok(SearchResult { books, total, page })
}

fn parse_books(html: &str) -> Vec<BookInfo> {
    let mut books = Vec::new();

    let card_re = regex::Regex::new(
        r#"<z-bookcard\s+(?P<attrs>[^>]*?)>(?P<content>[\s\S]*?)</z-bookcard>"#,
    )
    .unwrap();

    let attr_re = regex::Regex::new(r#"(\w+)\s*=\s*"([^"]*)""#).unwrap();
    let title_re = regex::Regex::new(r#"<div\s+slot="title">([\s\S]*?)</div>"#).unwrap();
    let author_re = regex::Regex::new(r#"<div\s+slot="author">([\s\S]*?)</div>"#).unwrap();
    let img_re = regex::Regex::new(r#"<img\s+[^>]*?data-src="([^"]*)""#).unwrap();

    for cap in card_re.captures_iter(html) {
        let attrs = &cap["attrs"];
        let content = &cap["content"];
        let mut book = BookInfo::default();

        for attr_cap in attr_re.captures_iter(attrs) {
            let key = &attr_cap[1];
            let val = attr_cap[2].to_string();
            match key {
                "id" => book.id = val,
                "isbn" => book.isbn = val,
                "href" => book.detail_url = format!("https://{}{}", client::ORIGIN_DOMAIN, val),
                "download" => book.download_url = format!("https://{}{}", client::ORIGIN_DOMAIN, val),
                "publisher" => book.publisher = val,
                "language" => book.language = val,
                "year" => book.year = val,
                "extension" => book.extension = val,
                "filesize" => book.file_size = val,
                "rating" => book.rating = val,
                "quality" => book.quality = val,
                _ => {}
            }
        }

        if let Some(t) = title_re.captures(content) {
            book.title = html_unescape(&t[1]);
        }
        if let Some(a) = author_re.captures(content) {
            book.author = html_unescape(&a[1]);
        }
        if let Some(i) = img_re.captures(content) {
            book.image_url = i[1].to_string();
        }

        books.push(book);
    }

    books
}

fn html_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&nbsp;", " ")
}

fn urlencoding(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}