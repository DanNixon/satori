mod config;
mod queue;
mod task;

use crate::{config::Config, queue::ArchiveTaskQueue};
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use satori_common::ArchiveCommand;
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, sync::Mutex};
use tracing::{error, info};

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

struct AppContext {
    storage: satori_storage::Provider,
    http_client: reqwest::Client,
}

struct AppState {
    context: Arc<AppContext>,
    queue: Arc<Mutex<ArchiveTaskQueue>>,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config)?;

    let context = Arc::new(AppContext {
        storage: config
            .storage
            .create_provider()
            .into_diagnostic()
            .wrap_err("Failed to create storage provider")?,
        http_client: reqwest::Client::new(),
    });

    let queue = Arc::new(Mutex::new(queue::ArchiveTaskQueue::load_or_new(
        &config.queue_file,
    )));
    let mut queue_process_interval = tokio::time::interval(config.interval);

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

    // Set up shared state
    let state = Arc::new(AppState {
        context: context.clone(),
        queue: queue.clone(),
    });

    // Configure HTTP server
    let app = Router::new()
        .route("/archive", post(handle_archive))
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

    // Run queue processing loop
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting");
                break;
            }
            _ = queue_process_interval.tick() => {
                let mut q = queue.lock().await;
                q.process_one(&context).await;
            }
        }
    }

    // Stop HTTP server
    info!("Stopping HTTP server");
    server_handle.abort();
    let _ = server_handle.await;

    Ok(())
}

#[tracing::instrument(skip_all)]
async fn handle_archive(
    State(state): State<Arc<AppState>>,
    Json(cmd): Json<ArchiveCommand>,
) -> impl IntoResponse {
    info!("Received archive command");

    // Convert the command into tasks
    let tasks = match &cmd {
        ArchiveCommand::EventMetadata(event) => {
            vec![task::ArchiveTask::EventMetadata(event.clone())]
        }
        ArchiveCommand::Segments(segments_cmd) => segments_cmd
            .segment_list
            .iter()
            .map(|segment| {
                task::ArchiveTask::CameraSegment(task::CameraSegment {
                    camera_name: segments_cmd.camera_name.clone(),
                    camera_url: segments_cmd.camera_url.clone(),
                    filename: segment.clone(),
                })
            })
            .collect(),
    };

    // Process each task synchronously
    for task in tasks {
        let mut queue = state.queue.lock().await;
        if let Err(e) = queue.process_task_sync(task, &state.context).await {
            error!("Failed to process archive task: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to process archive request".to_string());
        }
    }

    (StatusCode::OK, String::new())
}
