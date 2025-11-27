use core::str;

use axum::{
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use http::StatusCode;
use rust_embed::Embed;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

#[derive(Embed)]
#[folder = "public/"]
#[prefix = "static/"]
struct Assets;

async fn handler() -> Response {
    match Assets::get("static/index.html") {
        Some(html_data) => {
            let content = String::from_utf8(html_data.data.to_vec()).expect("should generate str");
            Html(content).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 NOT FOUND").into_response(),
    }
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let app = Router::new().route("/", get(handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    ewe_trace::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
