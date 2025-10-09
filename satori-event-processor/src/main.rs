mod config;
mod error;
mod event_set;
mod hls_client;
mod segments;

use crate::{
    config::{Config, TriggersConfig},
    event_set::EventSet,
};
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use satori_common::{TriggerCommand, mqtt::MqttClient};
use std::{net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{net::TcpListener, sync::Mutex};
use tracing::{debug, info};

const METRIC_TRIGGERS: &str = "satori_eventprocessor_triggers";
const METRIC_ACTIVE_EVENTS: &str = "satori_eventprocessor_active_events";
const METRIC_EXPIRED_EVENTS: &str = "satori_eventprocessor_expired_events";

struct AppState {
    events: Arc<Mutex<EventSet>>,
    trigger_config: Arc<TriggersConfig>,
}

/// Run the event processor.
#[derive(Clone, Parser)]
#[command(author, version = satori_common::version!(), about, long_about = None)]
pub(crate) struct Cli {
    /// Path to configuration file
    #[arg(short, long, env = "CONFIG_FILE", value_name = "FILE")]
    config: PathBuf,

    /// Address to listen on for HTTP API endpoints
    #[clap(long, env = "HTTP_SERVER_ADDRESS", default_value = "127.0.0.1:8080")]
    http_server_address: SocketAddr,

    /// Address to listen on for observability/metrics endpoints
    #[clap(long, env = "OBSERVABILITY_ADDRESS", default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config);

    // Set up and connect MQTT client
    let mut mqtt_client: MqttClient = config.mqtt.into();

    // Set up camera stream client
    let camera_client = self::hls_client::HlsClient::new(config.cameras);

    // Load existing or create new event state
    let events = Arc::new(Mutex::new(EventSet::load_or_new(
        &config.event_file,
        config.event_ttl,
    )));

    // Set up metrics server
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(cli.observability_address)
        .install()
        .into_diagnostic()
        .wrap_err("Failed to start prometheus metrics exporter")?;

    metrics::describe_counter!(METRIC_TRIGGERS, metrics::Unit::Count, "Trigger count");

    metrics::describe_gauge!(
        METRIC_ACTIVE_EVENTS,
        metrics::Unit::Count,
        "Number of active events"
    );

    metrics::describe_counter!(
        METRIC_EXPIRED_EVENTS,
        metrics::Unit::Count,
        "Processed events count"
    );

    // Set up shared state
    let state = Arc::new(AppState {
        events: events.clone(),
        trigger_config: Arc::new(config.triggers),
    });

    // Configure HTTP server
    let app = Router::new()
        .route("/trigger", post(handle_trigger))
        .with_state(state);

    let listener = TcpListener::bind(&cli.http_server_address)
        .await
        .into_diagnostic()
        .wrap_err("Failed to bind listener for HTTP server")?;

    info!("Starting HTTP server on {}", cli.http_server_address);

    // Spawn HTTP server task
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("HTTP server should run");
    });

    // Run event processing loop
    let mut process_interval = tokio::time::interval(config.interval);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting.");
                break;
            }
            _ = process_interval.tick() => {
                debug!("Processing events at interval");
                let mut events_lock = events.lock().await;
                events_lock.process(&camera_client, &mqtt_client).await;
            }
        }
    }

    // Stop HTTP server
    info!("Stopping HTTP server");
    server_handle.abort();
    let _ = server_handle.await;

    // Disconnect MQTT client
    mqtt_client.disconnect().await;

    Ok(())
}

#[tracing::instrument(skip_all)]
async fn handle_trigger(
    State(state): State<Arc<AppState>>,
    Json(cmd): Json<TriggerCommand>,
) -> impl IntoResponse {
    debug!("Trigger command: {:?}", cmd);

    let trigger = state.trigger_config.create_trigger(&cmd);
    let mut events = state.events.lock().await;
    events.trigger(&trigger);

    StatusCode::OK
}
