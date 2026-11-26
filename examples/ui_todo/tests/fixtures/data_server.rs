//! Serve one known record for native data client tests.
use foundation_http::shared::app::HttpApp;
use std::{net::TcpListener, sync::Arc};
use ui_todo::{server::Todos, todo_rpc, Todo, TodoStore};

#[foundation_core::valtron::valtron(seed = 0)]
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bind = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:0".into());
    let listener = TcpListener::bind(bind)?;
    let address = listener.local_addr()?;
    if !address.ip().is_loopback() {
        return Err("The data fixture requires a loopback address".into());
    }
    let mut app = HttpApp::new_serve();
    app.context().store(TodoStore::with_items(vec![Todo {
        id: "00000000-0000-4000-8000-000000000042".parse()?,
        text: "Buy milk".into(),
        done: false,
    }])?);
    let mut rpc = foundation_connectrpc::Router::new();
    todo_rpc::register_todos(&mut rpc, Arc::new(Todos));
    app.router.at(
        todo_rpc::procedure::GET_TODO,
        foundation_http::post(foundation_connectrpc::ConnectRpcServe::new(
            rpc.into_handler(),
        )),
    )?;
    println!("Open http://{address}/");
    app.server(&address.to_string())
        .serve_with_listener(&listener, &Arc::new(foundation_http::ShutdownSignal::new()));
    Ok(())
}
