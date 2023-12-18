mod config;
mod error;
mod queue;
mod task;

use crate::config::Config;
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use satori_common::mqtt::MqttClient;
use std::{net::SocketAddr, path::PathBuf};
use tracing::{debug, info};

const METRIC_QUEUE_LENGTH: &str = "satori_archiver_queue_length";
const METRIC_PROCESSED_TASKS: &str = "satori_archiver_processed_tasks";

/// Run the archiver.
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

struct Context {
    storage: satori_storage::Provider,
    http_client: reqwest::Client,
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config);

    let mut mqtt_client: MqttClient = config.mqtt.into();

    let context = Context {
        storage: config.storage.create_provider(),
        http_client: reqwest::Client::new(),
    };

    let mut queue = queue::ArchiveTaskQueue::load_or_new(&config.queue_file);
    let mut queue_process_interval = tokio::time::interval(config.interval);

    // Set up metrics server
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(cli.observability_address)
        .install()
        .expect("prometheus metrics exporter should be setup");

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

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting");
                break;
            }
            msg = mqtt_client.poll() => {
                if let Some(msg) = msg {
                    queue.handle_mqtt_message(msg);
                    // Immediately process the queue
                    queue.process(&context).await;
                }
            }
            _ = queue_process_interval.tick() => {
                debug!("Processing queue at interval");
                queue.process(&context).await;
            }
        }
    }

    // Disconnect MQTT client
    mqtt_client.disconnect().await;

    Ok(())
}
