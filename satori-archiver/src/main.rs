mod endpoints;
mod o11y;

use axum::{Router, routing::post};
use bytes::Bytes;
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use satori_storage::StorageConfig;
use std::{net::SocketAddr, path::PathBuf};
use tokio::net::TcpListener;
use tracing::info;
use url::Url;

/// Run the archiver.
#[derive(Clone, Parser)]
#[command(
    author,
    version = satori_common::version!(),
)]
pub(crate) struct Cli {
    /// Path to storage configuration file
    #[arg(short, long, env = "STORAGE_CONFIG_FILE", value_name = "FILE")]
    storage: PathBuf,

    /// Address to run the HTTP API on
    #[clap(long, env = "API_ADDRESS", default_value = "127.0.0.1:8000")]
    api_address: SocketAddr,

    /// Address to listen on for observability/metrics endpoints
    #[clap(long, env = "OBSERVABILITY_ADDRESS", default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,
}

#[derive(Clone)]
struct AppState {
    storage: satori_storage::Provider,
    http_client: reqwest::Client,
}

impl AppState {
    async fn get(&self, url: Url) -> reqwest::Result<Bytes> {
        let req = self.http_client.get(url).send().await?;
        req.bytes().await
    }
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: StorageConfig = satori_common::load_config_file(&cli.storage)?;

    // Set up metrics server
    crate::o11y::init();
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(cli.observability_address)
        .install()
        .into_diagnostic()
        .wrap_err("Failed to start prometheus metrics exporter")?;

    let state = AppState {
        storage: config
            .try_into()
            .into_diagnostic()
            .wrap_err("Failed to create storage provider")?,
        http_client: reqwest::Client::new(),
    };

    // Configure HTTP server
    let app = Router::new()
        .route("/event", post(crate::endpoints::handle_event_upload))
        .route(
            "/video/{camera}",
            post(crate::endpoints::handle_camera_segment_upload),
        )
        .with_state(state);

    let listener = TcpListener::bind(&cli.api_address)
        .await
        .into_diagnostic()
        .wrap_err("Failed to bind listener for API server")?;

    info!("Starting HTTP server on {}", cli.api_address);

    // Spawn HTTP server task
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("API server should run");
    });

    tokio::signal::ctrl_c()
        .await
        .into_diagnostic()
        .wrap_err("Failed to wait for exit signal")?;
    info!("Exiting");

    // Stop HTTP server
    info!("Stopping HTTP server");
    server_handle.abort();
    let _ = server_handle.await;

    Ok(())
}
