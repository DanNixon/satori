mod config;
mod queue;
mod task;

use crate::config::Config;
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use rdkafka::{ClientConfig, consumer::StreamConsumer};
use std::{net::SocketAddr, path::PathBuf};
use tracing::info;

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

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config)?;

    let kafka_consumer: StreamConsumer = ClientConfig::new()
        .set("bootstrap.servers", &config.kafka.brokers)
        .set("group.id", &config.kafka.consumer_group)
        .set("enable.auto.commit", "true")
        // .set("auto.offset.reset", "earliest")
        .create()
        .into_diagnostic()?;

    kafka_consumer
        .subscribe(&[&config.kafka.archive_command_topic])
        .into_diagnostic()?;

    let context = AppContext {
        storage: config
            .storage
            .create_provider()
            .into_diagnostic()
            .wrap_err("Failed to create storage provider")?,
        http_client: reqwest::Client::new(),
    };

    let mut queue = queue::ArchiveTaskQueue::load_or_new(&config.queue_file);
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

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting");
                break;
            }
            // TODO
            msg = kafka_consumer.poll() => {
                if let Some(msg) = msg {
                    queue.handle_kafka_message(msg);
                }
            }
            _ = queue_process_interval.tick() => {
                queue.process_one(&context).await;
            }
        }
    }

    Ok(())
}
