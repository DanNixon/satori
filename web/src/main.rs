mod config;
mod server;

use crate::config::Config;
use axum::{
    extract::{Path, State},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::{net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tracing::info;

/// Run the web server.
#[derive(Clone, Parser)]
#[command(author, version = satori_common::version!(), about, long_about = None)]
pub(crate) struct Cli {
    /// Path to configuration file
    #[arg(short, long, env = "CONFIG_FILE", value_name = "FILE")]
    config: PathBuf,

    /// Address to listen on for web endpoints
    #[clap(long, env = "HTTP_SERVER_ADDRESS", default_value = "127.0.0.1:8000")]
    http_server_address: SocketAddr,

    /// Address to listen on for observability/metrics endpoints
    #[clap(long, env = "OBSERVABILITY_ADDRESS", default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,
}

#[derive(Debug, Clone)]
struct Context {}

async fn get_camera_jpeg(State(state): State<Context>, Path(camera): Path<String>) -> Response {
    println!("get jpeg\n  state: {:?}\n  camera: {:?}", state, camera);
    "todo".into_response()
}

async fn get_camera_mjpeg(State(state): State<Context>, Path(camera): Path<String>) -> Response {
    println!("get mjpeg\n  state: {:?}\n  camera: {:?}", state, camera);
    "todo".into_response()
}

async fn get_camera_hls(State(state): State<Context>, Path(camera): Path<String>) -> Response {
    println!(
        "hls get plist\n  state: {:?}\n  camera: {:?}",
        state, camera
    );
    "todo".into_response()
}

async fn get_camera_hls_segment(
    State(state): State<Context>,
    Path((camera, segment)): Path<(String, String)>,
) -> Response {
    println!(
        "hls get segment\n  state: {:?}\n  camera: {:?}\n  segment: {:?}",
        state, camera, segment
    );
    "todo".into_response()
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config);

    // Set up metrics server
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(cli.observability_address)
        .install()
        .expect("prometheus metrics exporter should be setup");

    // TODO
    let state = Context {};
    let app = Router::new()
        .route("/:camera/jpeg", get(get_camera_jpeg))
        .route("/:camera/mjpeg", get(get_camera_mjpeg))
        .route("/:camera/hls/stream.m3u8", get(get_camera_mjpeg))
        .route("/:camera/hsl/:segment", get(get_camera_mjpeg))
        .with_state(state);

    // Configure HTTP server listener
    let listener = TcpListener::bind(&cli.http_server_address)
        .await
        .unwrap_or_else(|_| panic!("tcp listener should bind to {}", cli.http_server_address));

    // Start HTTP server
    info!("Starting HTTP server on {}", cli.http_server_address);
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    loop {
        tokio::select! {
            // TODO
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting");
                break;
            }
        }
    }

    // Stop server
    info!("Stopping HTTP server");
    server_handle.abort();
    let _ = server_handle.await;
}
