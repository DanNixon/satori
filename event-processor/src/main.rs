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
use std::{net::SocketAddr, path::PathBuf};
use tracing::{debug, error, info};

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
async fn main() -> Result<(), ()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config);

    let mqtt_control_topic = config.mqtt.topic();
    let mqtt_client = config.mqtt.build_client(false).await;

    let camera_client = self::hls_client::HlsClient::new(config.cameras);

    let mut events = EventSet::load_or_new(&config.event_file, config.event_ttl);

    let mut app_watcher = kagiyama::Watcher::<kagiyama::AlwaysReady>::default();
    {
        let mut registry = app_watcher.metrics_registry();
        let registry = registry.sub_registry_with_prefix("satori_eventprocessor");
        mqtt_client.register_metrics(registry);
        events.register_metrics(registry);
    }
    app_watcher.start_server(cli.observability_address).await;

    let mut mqtt_rx = mqtt_client.rx_channel();
    let mut process_interval = tokio::time::interval(config.interval);
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting.");
                break;
            }
            event = mqtt_rx.recv() => {
                match event {
                    Ok(mqtt_channel_client::Event::Rx(msg)) => {
                        if handle_mqtt_message(msg, &mut events, &config.triggers) {
                            // Immediately process events
                            events.process(&camera_client, &mqtt_client, mqtt_control_topic).await;
                        }
                    }
                    Ok(_) => {}
                    Err(err) => {
                        error!("MQTT channel error: {}", err);
                    }
                }
            }
            _ = process_interval.tick() => {
                debug!("Processing events at interval");
                events.process(&camera_client, &mqtt_client, mqtt_control_topic).await;
            }
        }
    }

    Ok(())
}

#[tracing::instrument(skip_all)]
fn handle_mqtt_message(
    msg: mqtt_channel_client::paho_mqtt::Message,
    events: &mut EventSet,
    trigger_config: &TriggersConfig,
) -> bool {
    let msg = serde_json::from_str::<satori_common::Message>(&msg.payload_str());
    if let Err(err) = msg {
        error!("Failed to parse MQTT message ({})", err);
        return false;
    }

    if let satori_common::Message::TriggerCommand(cmd) = msg.unwrap() {
        debug!("Trigger command: {:?}", cmd);
        let trigger = trigger_config.create_trigger(&cmd);
        events.trigger(&trigger);
        true
    } else {
        false
    }
}
