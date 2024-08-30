use axum::{response::Html, routing::get, Router};

async fn handler() -> Html<&'static str> {
    Html("<h1>Wello World</h1>")
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() {
    let app = Router::new().route("/", get(handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3200")
        .await
        .unwrap();

    println!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
