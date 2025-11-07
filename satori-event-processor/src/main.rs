mod api;
mod archive;
mod config;
mod event_set;
mod hls_client;
mod o11y;
mod segments;

use self::hls_client::HlsClient;
use crate::{
    archive::{retry_queue::ArchiveRetryQueue, tasks::ArchiveTask},
    config::{Config, TriggersConfig},
    event_set::EventSet,
    o11y::ArchiveTaskResult,
};
use axum::{Router, routing::post};
use clap::Parser;
use miette::{Context, IntoDiagnostic};
use object_store::local::LocalFileSystem;
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    net::TcpListener,
    sync::{Mutex, watch::Receiver},
    task::JoinSet,
    time::Instant,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use url::Url;

#[derive(Clone)]
struct AppState {
    events: Arc<Mutex<EventSet>>,
    trigger_config: TriggersConfig,
    process_trigger: tokio::sync::watch::Sender<Instant>,
}

/// Run the event processor.
#[derive(Clone, Parser)]
#[command(
    author,
    version = satori_common::version!(),
)]
pub(crate) struct Cli {
    /// Path to configuration file
    #[arg(short, long, env = "CONFIG_FILE", value_name = "FILE")]
    config: PathBuf,

    /// Address to listen on for HTTP API endpoints
    #[clap(long, env = "HTTP_SERVER_ADDRESS", default_value = "127.0.0.1:8000")]
    http_server_address: SocketAddr,

    /// Address to listen on for observability/metrics endpoints
    #[clap(long, env = "OBSERVABILITY_ADDRESS", default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    // Parse CLI and load configuration file
    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config)?;

    o11y::init(cli.observability_address)?;

    let shutdown = CancellationToken::new();

    // Create camera stream client
    let camera_client = HlsClient::new(config.cameras);

    let state_store =
        Arc::new(LocalFileSystem::new_with_prefix(config.state_store).into_diagnostic()?);

    // Load existing or create new event state
    let events = Arc::new(Mutex::new(
        EventSet::new(state_store.clone(), config.event_ttl).await,
    ));

    // Load existing or create new queue of failed archive tasks
    let archive_retry_queue = ArchiveRetryQueue::new(
        state_store,
        chrono::Duration::from_std(config.archive_failed_task_ttl)
            .into_diagnostic()
            .wrap_err("Archive failed task duration out of range")?,
    )
    .await;

    // Channel for triggering event processing
    let (event_process_trigger_tx, event_process_trigger_rx) =
        tokio::sync::watch::channel(Instant::now());

    // Channel for processing archive tasks
    let (archive_task_tx, archive_task_rx) = tokio::sync::mpsc::unbounded_channel::<ArchiveTask>();

    // Channel for queueing failed archive tasks for reattempt
    let (failed_archive_task_tx, failed_archive_task_rx) =
        tokio::sync::mpsc::unbounded_channel::<ArchiveTask>();

    let mut tasks = JoinSet::new();

    // Set up shared state
    let state = AppState {
        events: events.clone(),
        trigger_config: config.triggers,
        process_trigger: event_process_trigger_tx,
    };

    // Configure HTTP server
    let app = Router::new()
        .route("/trigger", post(api::handle_trigger))
        .with_state(state);

    let listener = TcpListener::bind(&cli.http_server_address)
        .await
        .into_diagnostic()
        .wrap_err("Failed to bind listener for HTTP server")?;

    info!("Starting HTTP server on {}", cli.http_server_address);

    // Spawn HTTP server task
    {
        let shutdown = shutdown.clone();
        let _ = tasks.spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(shutdown.cancelled_owned())
                .await
                .expect("HTTP server should run");
        });
    }

    // Spawn event processing task
    {
        let shutdown = shutdown.clone();
        let archive_task_tx = archive_task_tx.clone();
        let _ = tasks.spawn(async move {
            process_events(
                shutdown,
                events,
                camera_client,
                &config.storage_api_urls,
                config.event_process_interval,
                event_process_trigger_rx,
                archive_task_tx,
            )
            .await;
        });
    }

    // Spawn archive task retry queue task
    {
        let shutdown = shutdown.clone();
        let _ = tasks.spawn(async move {
            process_archive_retry_queue(
                shutdown,
                archive_retry_queue,
                config.archive_retry_interval,
                failed_archive_task_rx,
                archive_task_tx,
            )
            .await;
        });
    }

    // Spawn archive submission task
    {
        let shutdown = shutdown.clone();
        let _ = tasks.spawn(async move {
            process_archive_submission(shutdown, archive_task_rx, failed_archive_task_tx).await;
        });
    }

    // Wait for exit signal
    tokio::select! {
        Ok(_) = tokio::signal::ctrl_c() => {
            shutdown.cancel();
        }
        _ = shutdown.cancelled() => {}
    }
    info!("Exiting.");

    // Stop tasks
    let results = tasks.join_all().await;
    info!("Task results: {results:?}");

    Ok(())
}

async fn process_events(
    shutdown: CancellationToken,
    events: Arc<Mutex<EventSet>>,
    camera_client: HlsClient,
    storage_api_urls: &[Url],
    interval: Duration,
    mut trigger_rx: Receiver<Instant>,
    task_tx: tokio::sync::mpsc::UnboundedSender<ArchiveTask>,
) {
    let mut interval = tokio::time::interval(interval);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                break;
            }
            _ = interval.tick() => {
                info!("Processing events on interval");
                let mut events = events.lock().await;
                if let Err(e) = events.process(&camera_client, storage_api_urls, &task_tx).await {
                    error!("Event processing failed: {e}");
                    shutdown.cancel();
                }
            }
            Ok(_) = trigger_rx.changed() => {
                info!("Processing events on trigger");
                let mut events = events.lock().await;
                if let Err(e) = events.process(&camera_client, storage_api_urls, &task_tx).await {
                    error!("Event processing failed: {e}");
                    shutdown.cancel();
                }
            }
        }
    }

    info!("Shutting down event processing task");

    // Save event set on exit
    let events = events.lock().await;
    events.save().await;
}

async fn process_archive_retry_queue(
    shutdown: CancellationToken,
    mut queue: ArchiveRetryQueue,
    interval: Duration,
    mut failed_task_rx: tokio::sync::mpsc::UnboundedReceiver<ArchiveTask>,
    task_tx: tokio::sync::mpsc::UnboundedSender<ArchiveTask>,
) {
    let mut interval = tokio::time::interval(interval);

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                break;
            }
            _ = interval.tick() => {
                info!("Processing failed archive task queue");
                if let Err(e) = queue.process(&task_tx).await {
                    error!("Archive task retry queue processing failed: {e}");
                    shutdown.cancel();
                }
            }
            Some(task) = failed_task_rx.recv() => {
                queue.push(task);
            }
        }
    }

    info!("Shutting down failed archive task queue processing task");

    // Save retry queue set on exit
    queue.save().await;
}

async fn process_archive_submission(
    shutdown: CancellationToken,
    mut task_rx: tokio::sync::mpsc::UnboundedReceiver<ArchiveTask>,
    failed_task_tx: tokio::sync::mpsc::UnboundedSender<ArchiveTask>,
) {
    // TODO: support concurrent archive requests

    let http_client = reqwest::Client::new();

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                break;
            }
            Some(task) = task_rx.recv() => {
                if let Err(e) = task.execute(&http_client).await {
                    warn!("Failed to run archive task {task:?}: {e}");
                    o11y::inc_archive_task_metric(&task.api_url, ArchiveTaskResult::Failure);
                    if let Err(e) = failed_task_tx.send(task) {
                        error!("Failed to send archive task on channel: {e}");
                        shutdown.cancel();
                    }
                } else {
                    o11y::inc_archive_task_metric(&task.api_url, ArchiveTaskResult::Success);
                }
            }
        }
    }
}
