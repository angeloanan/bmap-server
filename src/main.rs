#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(clippy::cargo)]
#![warn(clippy::perf)]
#![warn(clippy::complexity)]
#![warn(clippy::style)]
use std::{net::IpAddr, path::PathBuf};

use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode},
    response::IntoResponse,
    routing::get,
};
use axum_server::tls_rustls::RustlsConfig;
use clap::Parser;
use tower_http::{
    services::ServeDir,
    trace::{self, TraceLayer},
};
use tracing::{Level, debug, error, info, instrument, trace};

#[derive(Clone, Debug)]
struct AppState {
    client: reqwest::Client,
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

    // Traverse to the web directory - the base directory of what we're serving
    let mut web_path = path.clone();
    web_path.push("web");

    // Forms the path to the index file for verification purposes only
    // Internally, a BLUEMAP_PATH is valid if it contains `web/index.html` in the folder
    let mut index_file = web_path.clone();
    index_file.push("index.html");
    assert!(
        index_file.is_file(),
        "Provided BLUEMAP_PATH does not look like a valid bluemap data directory. Did you point it to the root directory? Provided path: {}",
        path.display()
    );

    info!("Using Bluemap data directory: {}", path.display());
    let serve_directory = ServeDir::new(web_path)
        .precompressed_gzip()
        .precompressed_zstd();

    trace!("Building reverse proxy client");
    let state = AppState {
        client: reqwest::ClientBuilder::new()
            .user_agent(concat!(
                "Mozilla/5.0 (compatible; {}/{}; +{})",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                env!("CARGO_PKG_HOMEPAGE")
            ))
            .build()
            .unwrap(),
        bluemap_origin: format!(
            "{}:{}",
            args.bluemap_host.unwrap(),
            args.bluemap_port.unwrap()
        )
        .leak(),
    };

    let app = Router::new()
        .route("/maps/{world_name}/live/{*any}", get(proxy_live_data))
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
    req: Request,
) -> axum::response::Response {
    let complete_path = req
        .uri()
        .path_and_query()
        .map_or(req.uri().path(), |v| v.as_str());
    trace!("Proxying request of {complete_path}");

    let data = match app_state
        .client
        .get(format!(
            "http://{}{complete_path}",
            app_state.bluemap_origin
        ))
        .send()
        .await
    {
        Ok(data) => data,
        Err(e) => {
            error!("Error while fetching data from Bluemap: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let response_builder = Response::builder().status(data.status());
    // Here the mapping of headers is required due to reqwest and axum differ on the http crate versions
    let mut headers = HeaderMap::with_capacity(data.headers().len());
    headers.extend(data.headers().into_iter().map(|(name, value)| {
        let name = HeaderName::from_bytes(name.as_ref()).unwrap();
        let value = HeaderValue::from_bytes(value.as_ref()).unwrap();
        (name, value)
    }));

    response_builder
        .body(Body::from_stream(data.bytes_stream()))
        // This unwrap is fine because the body is empty here
        .unwrap()
}
