mod config;
// mod task;

use crate::config::Config;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::post,
};
use bytes::Bytes;
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use satori_common::{ArchiveSegmentCommand, Event};
use satori_storage::StorageProvider;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::net::TcpListener;
use tracing::{info, warn};
use url::Url;

const METRIC_QUEUE_LENGTH: &str = "satori_archiver_queue_length";
const METRIC_PROCESSED_TASKS: &str = "satori_archiver_processed_tasks";

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
    async fn get(&self, url: Url) -> miette::Result<Bytes> {
        let req = self.http_client.get(url).send().await.into_diagnostic()?;
        req.bytes().await.into_diagnostic()
    }
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config)?;

    // Set up metrics server
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(cli.observability_address)
        .install()
        .into_diagnostic()
        .wrap_err("Failed to start prometheus metrics exporter")?;

    metrics::describe_gauge!(
        METRIC_QUEUE_LENGTH,
        metrics::Unit::Count,
        "Number of tasks in queue"
    );

    metrics::describe_counter!(
        METRIC_PROCESSED_TASKS,
        metrics::Unit::Count,
        "Finished task count"
    );

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
        .route("/event", post(handle_event_upload))
        .route(
            "/video/{camera}/{filename}",
            post(handle_camera_segment_upload),
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

#[tracing::instrument(skip_all)]
async fn handle_event_upload(
    State(state): State<Arc<AppState>>,
    Json(event): Json<Event>,
) -> impl IntoResponse {
    info!("Saving event");

    match state.storage.put_event(&event).await {
        Ok(_) => (StatusCode::OK, String::new()),
        Err(e) => {
            warn!("Failed to store event with error {e}");
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        }
    }
}

#[tracing::instrument(skip_all)]
async fn handle_camera_segment_upload(
    State(state): State<Arc<AppState>>,
    Path(camera): Path<String>,
    Json(cmd): Json<ArchiveSegmentCommand>,
) -> impl IntoResponse {
    info!("Saving segment");

    let filename = cmd
        .segment_url
        .path_segments()
        .and_then(|segments| segments.last())
        .unwrap();
    let filename = PathBuf::from(filename);

    let data = state.get(cmd.segment_url).await.unwrap();

    state
        .storage
        .put_segment(&camera, &filename, data)
        .await
        .into_diagnostic()
        .unwrap();

    (StatusCode::OK, String::new())
}
