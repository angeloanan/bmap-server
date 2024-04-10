#![feature(absolute_path)]
use std::{net::IpAddr, path::PathBuf};

use axum::{
    body::Body,
    extract::{Request, State},
    http::{StatusCode, Uri},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use tower_http::{
    services::ServeDir,
    trace::{self, TraceLayer},
};
use tracing::{debug, info, instrument, trace, Level};

use hyper_util::{client::legacy::connect::HttpConnector, rt::TokioExecutor};

#[derive(Clone, Debug)]
struct AppState {
    client: hyper_util::client::legacy::Client<HttpConnector, Body>,
    bluemap_origin: &'static str,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to Bluemap's data directory
    bluemap_dir: String,

    /// Host to listen
    #[arg(long, default_value = "0.0.0.0")]
    host: Option<IpAddr>,
    /// Port to listen
    #[arg(short, long, default_value = "31283")]
    port: Option<u16>,

    /// Bluemap's Live Server host
    #[arg(long, default_value = "127.0.0.1")]
    bluemap_host: Option<IpAddr>,
    /// Bluemap's Live Server port
    #[arg(long, default_value = "8100")]
    bluemap_port: Option<u16>,

    /// TLS certificate file - If not provided, server will run without TLS
    #[arg(long)]
    tls_cert: Option<String>,
    /// TLS key file - If not provided, server will run without TLS
    #[arg(long)]
    tls_key: Option<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let args = Args::parse();
    trace!("Arguments passed successfully");

    let path = std::path::absolute(PathBuf::from(args.bluemap_dir)).unwrap();
    trace!("Fully extended path: {}", path.display());

    assert!(
        path.is_dir(),
        "Provided BLUEMAP_PATH is not a valid directory: {}",
        path.display()
    );
    trace!("BLUEMAP_PATH is a valid directory!");

    let mut web_path = path.clone();
    web_path.push("web");

    let mut index_file = web_path.clone();
    index_file.push("index.html");
    assert!(
        index_file.is_file(),
        "Provided BLUEMAP_PATH does not look like a valid bluemap data directory. Did you point it to the root directory? Provided path: {}", path.display()
    );

    info!("Using Bluemap data directory: {}", path.display());
    let serve_directory = ServeDir::new(web_path).precompressed_gzip();

    trace!("Building reverse proxy client");
    let state = AppState {
        client: hyper_util::client::legacy::Client::<(), ()>::builder(TokioExecutor::new())
            .build(HttpConnector::new()),
        bluemap_origin: format!(
            "{}:{}",
            args.bluemap_host.unwrap(),
            args.bluemap_port.unwrap()
        )
        .leak(),
    };

    let app = Router::new()
        .route("/maps/:world_name/live", get(proxy_live_data))
        .with_state(state)
        .nest_service("/", serve_directory)
        .layer(
            TraceLayer::new_for_http()
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        );

    let use_tls = args.tls_cert.is_some() && args.tls_key.is_some();
    info!("Using TLS: {}", use_tls);

    debug!("Trying to bind to port {}", args.port.unwrap());
    let listener = tokio::net::TcpListener::bind((args.host.unwrap(), args.port.unwrap()))
        .await
        .unwrap();
    info!("Listening on http://{}", listener.local_addr().unwrap());

    if use_tls {
        let tls_config = RustlsConfig::from_pem_file(args.tls_cert.unwrap(), args.tls_key.unwrap())
            .await
            .unwrap();
        axum_server::from_tcp_rustls(listener.into_std().unwrap(), tls_config)
            .serve(app.into_make_service())
            .await
            .unwrap();
    } else {
        axum::serve(listener, app).await.unwrap();
    }
}

#[instrument]
async fn proxy_live_data(
    State(app_state): State<AppState>,
    mut req: Request,
) -> Result<Response, StatusCode> {
    let complete_path = req
        .uri()
        .path_and_query()
        .map(|v| v.as_str())
        .unwrap_or(req.uri().path());
    trace!("Proxying request of {complete_path}");

    let bmap_uri = format!("http://{}{complete_path}", app_state.bluemap_origin);
    *req.uri_mut() = Uri::try_from(bmap_uri).unwrap();

    Ok(app_state
        .client
        .request(req)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .into_response())
}
