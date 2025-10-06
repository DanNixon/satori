mod config;
mod error;
mod event_set;
mod hls_client;
mod segments;

use crate::{
    config::{Config, TriggersConfig},
    event_set::EventSet,
};
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use satori_common::mqtt::{MqttClient, PublishExt};
use std::{net::SocketAddr, path::PathBuf};
use tracing::{debug, error, info};

const METRIC_TRIGGERS: &str = "satori_eventprocessor_triggers";
const METRIC_ACTIVE_EVENTS: &str = "satori_eventprocessor_active_events";
const METRIC_EXPIRED_EVENTS: &str = "satori_eventprocessor_expired_events";

/// Run the event processor.
#[derive(Clone, Parser)]
#[command(author, version = satori_common::version!(), about, long_about = None)]
pub(crate) struct Cli {
    /// Path to configuration file
    #[arg(short, long, env = "CONFIG_FILE", value_name = "FILE")]
    config: PathBuf,

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
    let mut events = EventSet::load_or_new(&config.event_file, config.event_ttl);

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

    // Run event loop
    let mut process_interval = tokio::time::interval(config.interval);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting.");
                break;
            }
            msg = mqtt_client.poll() => {
                if let Some(msg) = msg {
                    if handle_mqtt_message(msg, &mut events, &config.triggers) {
                        // Immediately process events
                        events.process(&camera_client, &mqtt_client).await;
                    }
                }
            }
            _ = process_interval.tick() => {
                debug!("Processing events at interval");
                events.process(&camera_client, &mqtt_client).await;
            }
        }
    }

    // Disconnect MQTT client
    mqtt_client.disconnect().await;

    Ok(())
}

#[tracing::instrument(skip_all)]
fn handle_mqtt_message(
    msg: rumqttc::Publish,
    events: &mut EventSet,
    trigger_config: &TriggersConfig,
) -> bool {
    match msg.try_payload_from_json::<satori_common::Message>() {
        Ok(satori_common::Message::TriggerCommand(cmd)) => {
            debug!("Trigger command: {:?}", cmd);
            let trigger = trigger_config.create_trigger(&cmd);
            events.trigger(&trigger);
            true
        }
        Ok(_) => false,
        Err(e) => {
            error!("Failed to parse MQTT message ({})", e);
            false
        }
    }
}
