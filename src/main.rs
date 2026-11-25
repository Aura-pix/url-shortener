use url_shortener::{app, AppState};

#[tokio::main]
async fn main() {
    let base_url = std::env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_owned());
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to 0.0.0.0:3000");

    println!("URL shortener listening on http://localhost:3000");
    axum::serve(listener, app(AppState::new(base_url)))
        .await
        .expect("server failed");
}
