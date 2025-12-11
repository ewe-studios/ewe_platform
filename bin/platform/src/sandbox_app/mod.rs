use core::str;

use axum::{
    body,
    extract::Request,
    response::{Html, IntoResponse, Response},
    routing::get,
    Router,
};
use http::StatusCode;
use rust_embed::Embed;
use tracing::Level;
use tracing_subscriber::FmtSubscriber;

use foundation_core::megatron::jsrum::package_request_handler;
use foundation_nostd::embeddable::{EmbeddableFile, FileData};
use foundation_runtimes::js_runtimes::JSHostRuntime;

type BoxedError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Embed)]
#[folder = "src/sandbox_app/public/"]
#[prefix = "public/"]
struct Public;

async fn index_handler() -> Response {
    match package_request_handler("megatron".into(), "packages/public/index.html") {
        Some((file_content, mime_type)) => {
            tracing::info!("Pulling from package provider: index: /index.html");
            if mime_type
                .map(|t| t.as_str() == "text/html")
                .unwrap_or(false)
            {
                return Html(file_content).into_response();
            }
            file_content.into_response()
        }
        None => match Public::get("public/index.html") {
            Some(html_data) => {
                tracing::info!("Falling back to public index: public/index.html");
                let content =
                    String::from_utf8(html_data.data.to_vec()).expect("should generate str");
                Html(content).into_response()
            }
            None => (StatusCode::NOT_FOUND, "404 NOT FOUND").into_response(),
        },
    }
}

async fn megatron_handler(req: Request) -> Response {
    let request_path = req.uri().path();
    tracing::info!(
        "[MegatronHandler] Received request for path: {}",
        request_path
    );
    match package_request_handler("/megatron".into(), request_path) {
        Some((file_content, mime_type)) => {
            if mime_type
                .map(|t| t.as_str() == "text/html")
                .unwrap_or(false)
            {
                return Html(file_content).into_response();
            }

            if request_path.ends_with(".wasm") {
                if let Ok(response) = Response::builder()
                    .status(StatusCode::OK)
                    .header("CONTENT-TYPE", "application/wasm")
                    .body(body::Body::from(file_content.clone()))
                {
                    return response;
                }
            }

            file_content.into_response()
        }
        None => (StatusCode::NOT_FOUND, "404 NOT FOUND").into_response(),
    }
}

async fn jsruntime_handler(req: Request) -> Response {
    let request_path = req.uri().path();
    tracing::info!("[JSHandler] Received request for path: {}", request_path);

    let runtime = JSHostRuntime::default();
    match runtime
        .read_u8()
        .map(|data| String::from_utf8(data))
        .ok()
        .map(|value| value.into_response()) {
            Ok(response) => response
            Err(err) => {
                tracing::error!("Failed to fetch contents due to: {:?}", &err);
                (StatusCode::INTERNAL_SERVER_ERROR, "{:?}".format(err)).into_response()
            }
        }
}

async fn public_handler(req: Request) -> Response {
    let request_path = req.uri().path();
    tracing::info!(
        "[PublicHandler] Received request for path: {}",
        request_path
    );
    match Public::get(request_path.strip_prefix("/").unwrap_or(request_path)) {
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
        .route("/runtime/mega.js", get(jsruntime_handler))
        .route("/public/*path", get(public_handler))
        .route("/megatron/*path", get(megatron_handler));

    let listener = tokio::net::TcpListener::bind(format!("{service_addr}:{service_port}"))
        .await
        .unwrap();

    ewe_trace::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.map_err(Box::new)?;

    Ok(())
}
