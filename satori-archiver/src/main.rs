mod config;
mod endpoints;
mod metrics;

use crate::config::Config;
use axum::{Router, routing::post};
use bytes::Bytes;
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
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
    /// Path to configuration file
    #[arg(short, long, env = "CONFIG_FILE", value_name = "FILE")]
    config: PathBuf,

    /// Address to listen on for observability/metrics endpoints
    #[clap(long, env = "OBSERVABILITY_ADDRESS", default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,
}

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
    let config: Config = satori_common::load_config_file(&cli.config)?;

    // Set up metrics server
    crate::metrics::init();
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(cli.observability_address)
        .install()
        .into_diagnostic()
        .wrap_err("Failed to start prometheus metrics exporter")?;

    let state = Arc::new(AppState {
        storage: config
            .storage
            .create_provider()
            .into_diagnostic()
            .wrap_err("Failed to create storage provider")?,
        http_client: reqwest::Client::new(),
    });

    // Configure HTTP server
    let app = Router::new()
        .route("/event", post(crate::endpoints::handle_event_upload))
        .route(
            "/video/{camera}/{filename}",
            post(crate::endpoints::handle_camera_segment_upload),
        )
        .with_state(state);

    let listener = TcpListener::bind(&config.http_server_address)
        .await
        .into_diagnostic()
        .wrap_err("Failed to bind listener for HTTP server")?;

    info!("Starting HTTP server on {}", config.http_server_address);

    // Spawn HTTP server task
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("HTTP server should run");
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
