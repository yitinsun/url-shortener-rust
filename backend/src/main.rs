#[tokio::main]
async fn main() {
    if let Err(err) = url_shortener::serve().await {
        eprintln!("url-shortener failed: {err}");
        std::process::exit(1);
    }
}
