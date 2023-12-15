mod config;
mod server;

use crate::config::Config;
use clap::Parser;
use std::{net::SocketAddr, path::PathBuf};
use tracing::info;

/// Run the camera agent.
///
/// Handles restreaming a single camera as HLS with history.
#[derive(Clone, Parser)]
#[command(author, version = satori_common::version!(), about, long_about = None)]
pub(crate) struct Cli {
    /// Path to configuration file
    #[arg(short, long, env = "CONFIG_FILE", value_name = "FILE")]
    config: PathBuf,

    /// Address to listen on for observability/metrics endpoints
    #[clap(long, env = "HTTP_SERVER_ADDRESS", default_value = "127.0.0.1:8000")]
    http_server_address: SocketAddr,

    /// Address to listen on for observability/metrics endpoints
    #[clap(long, env = "OBSERVABILITY_ADDRESS", default_value = "127.0.0.1:9090")]
    observability_address: SocketAddr,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: Config = satori_common::load_config_file(&cli.config);

    // TODO
    println!("{:?}", config);

    // Set up observability server
    let mut app_watcher = kagiyama::Watcher::<kagiyama::AlwaysReady>::default();
    app_watcher.start_server(cli.observability_address).await;

    // Register metrics
    {
        let mut registry = app_watcher.metrics_registry();
        let registry = registry.sub_registry_with_prefix("satori_web");

        // TODO
    }

    // TODO

    loop {
        tokio::select! {
            // TODO
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting");
                break;
            }
        }
    }

    // Stop observability server
    app_watcher.stop_server().unwrap();
}
