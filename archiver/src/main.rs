mod config;
mod error;
mod queue;
mod task;

use crate::config::Config;
use clap::Parser;
use satori_common::camera_config::CamerasConfig;
use std::{net::SocketAddr, path::PathBuf};
use tracing::{debug, info};

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

struct Context {
    storage: satori_storage::Provider,
    cameras: CamerasConfig,
    http_client: reqwest::Client,
}

#[tokio::main]
async fn main() -> Result<(), ()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config);

    let mqtt_client = config.mqtt.build_client(false).await;

    let context = Context {
        storage: config.storage.create_provider(),
        cameras: config.cameras,
        http_client: reqwest::Client::new(),
    };

    let mut queue = queue::ArchiveTaskQueue::load_or_new(&config.queue_file);
    let mut queue_process_interval = tokio::time::interval(config.interval);

    let mut app_watcher = kagiyama::Watcher::<kagiyama::AlwaysReady>::default();
    {
        let mut registry = app_watcher.metrics_registry();
        let registry = registry.sub_registry_with_prefix("satori_archiver");
        mqtt_client.register_metrics(registry);
        queue.register_metrics(registry);
    }

    app_watcher.start_server(cli.observability_address).await;
    let mut mqtt_rx = mqtt_client.rx_channel();
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting");
                break;
            }
            Ok(mqtt_channel_client::Event::Rx(msg)) = mqtt_rx.recv() => {
                queue.handle_mqtt_message(msg);
                // Immediately process the queue
                queue.process(&context).await;
            }
            _ = queue_process_interval.tick() => {
                debug!("Processing queue at interval");
                queue.process(&context).await;
            }
        }
    }

    Ok(())
}
