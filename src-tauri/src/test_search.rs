use zlibrary_core::client;
use zlibrary_core::download;
use zlibrary_core::search;

#[tokio::main]
async fn main() {
    zlibrary_core::client::init_resolver().await;

    let result = search::search_books("python", 1).await.expect("search");
    if result.books.is_empty() {
        eprintln!("no results");
        return;
    }

    let book = &result.books[0];
    eprintln!("book: {} by {}", book.title, book.author);
    eprintln!("  download_url: {}", book.download_url);

    let rewritten = client::rewrite_url(&book.download_url);
    eprintln!("  rewritten: {rewritten}");

    eprintln!("\n--- testing follow_redirect ---");
    let resp = client::get_with_challenge(&rewritten).await.expect("get_with_challenge");
    let status = resp.status();
    eprintln!("  status: {status}");
    eprintln!("  final url: {}", resp.url());

    if status.is_redirection() {
        let location = resp.headers().get("location").map(|v| v.to_str().unwrap_or("?"));
        eprintln!("  location: {:?}", location);
    } else if status.is_success() {
        let body = resp.text().await.expect("body");
        eprintln!("  body({}b): {}", body.len(), &body[..200.min(body.len())]);
    }

    eprintln!("\n--- testing full download flow ---");
    match download::download_book(book).await {
        Ok(path) => eprintln!("SUCCESS! saved to: {:?}", path),
        Err(e) => eprintln!("download error: {e}"),
    }
}