use core::str;

use axum::{
    extract::Request,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use http::StatusCode;
use rust_embed::Embed;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use foundation_core::megatron::jsdom::package_request_handler;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Embed)]
#[folder = "src/sandbox_app/public/"]
#[prefix = "public/"]
struct Public;

async fn index_handler() -> Response {
    match Public::get("public/index.html") {
        Some(html_data) => {
            let content = String::from_utf8(html_data.data.to_vec()).expect("should generate str");
            Html(content).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 NOT FOUND").into_response(),
    }
}

async fn megatron_handler(req: Request) -> Response {
    let request_path = req.uri().path();
    tracing::info!(
        "[MegatronHandler] Received request for path: {}",
        request_path
    );
    match package_request_handler("megatron".into(), request_path) {
        Some(file_content) => {
            let content =
                String::from_utf8(file_content.data.to_vec()).expect("should generate str");
            Html(content).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 NOT FOUND").into_response(),
    }
}

async fn public_handler(req: Request) -> Response {
    let request_path = req.uri().path();
    tracing::info!(
        "[PublicHandler] Received request for path: {}",
        request_path
    );
    match Public::get(
        request_path
            .strip_prefix("/")
            .unwrap_or_else(|| request_path),
    ) {
        Some(html_data) => {
            let content = String::from_utf8(html_data.data.to_vec()).expect("should generate str");
            Html(content).into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 NOT FOUND").into_response(),
    }
}

pub fn register(command: clap::Command) -> clap::Command {
    command.subcommand(
        clap::Command::new("sandbox_app")
            .about("runs a local server for running our sandbox applications and demos")
            .arg(
                clap::Arg::new("service_addr")
                    .long("service_addr")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(String))
                    .default_value("0.0.0.0"),
            )
            .arg(
                clap::Arg::new("service_port")
                    .long("service_port")
                    .action(clap::ArgAction::Set)
                    .value_parser(clap::value_parser!(usize))
                    .default_value("3080"),
            ),
    )
}

pub async fn run(args: &clap::ArgMatches) -> std::result::Result<(), BoxedError> {
    let service_addr = args
        .get_one::<String>("service_addr")
        .expect("should have service address");

    let service_port = args
        .get_one::<usize>("service_port")
        .expect("should have service port");

    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default trace subscriber failed");

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/public/*path", get(public_handler))
        .route("/megatron/*path", get(megatron_handler));

    let listener = tokio::net::TcpListener::bind(format!("{}:{}", service_addr, service_port))
        .await
        .unwrap();

    ewe_trace::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .await
        .map_err(|err| Box::new(err))?;

    Ok(())
}
