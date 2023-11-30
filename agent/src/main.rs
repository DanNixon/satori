mod config;
mod ffmpeg;
mod server;

use clap::Parser;
use kagiyama::prometheus::{metrics::gauge::Gauge, registry::Unit};
use std::{fs, net::SocketAddr, path::PathBuf, time::Duration};
use tracing::{debug, info, warn};

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
    let config: config::Config = satori_common::load_config_file(&cli.config);

    info!("FFmpeg version: {}", ffmpeg::get_ffmpeg_version());

    // Set up observability server
    let mut app_watcher = kagiyama::Watcher::<kagiyama::AlwaysReady>::default();
    app_watcher.start_server(cli.observability_address).await;

    // Counter for number of files processed
    let segments_metric: Gauge = Gauge::default();
    let disk_usage_metric: Gauge = Gauge::default();
    let ffmpeg_invocations_metric: Gauge = Gauge::default();

    // Register metrics
    {
        let mut registry = app_watcher.metrics_registry();
        let registry = registry.sub_registry_with_prefix("satori_agent");

        registry.register(
            "segments",
            "Number of MPEG-TS segments generated",
            segments_metric.clone(),
        );

        registry.register_with_unit(
            "disk_usage",
            "Disk usage of this camera's output video directory",
            Unit::Bytes,
            disk_usage_metric.clone(),
        );

        registry.register(
            "ffmpeg_invocations",
            "Number of times ffmpeg has been invoked",
            ffmpeg_invocations_metric.clone(),
        );
    }

    // Create video output directory
    fs::create_dir_all(&config.video_directory).expect("should be able to create output directory");

    // Generate a random filename for the frame JPEG
    let frame_file = tempfile::Builder::new()
        .prefix("satori-")
        .suffix(".jpg")
        .tempfile()
        .unwrap();

    // Start HTTP server
    let server_handle = server::run(
        cli.http_server_address,
        config.clone(),
        frame_file.path().to_owned(),
    );

    // Start streamer
    let streamer =
        ffmpeg::Streamer::new(config.clone(), frame_file.path(), ffmpeg_invocations_metric);
    let streamer_handle = streamer.start().await;

    let mut metrics_interval = tokio::time::interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = metrics_interval.tick() => {
                update_segment_count_metric(&segments_metric, &config);
                update_disk_usage_metric(&disk_usage_metric, &config);
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting");
                break;
            }
        }
    }

    // Stop streamer
    streamer.stop().await;
    streamer_handle.await.unwrap();

    // Terminate HTTP server
    server_handle.abort();
    let _ = server_handle.await;

    // Stop observability server
    app_watcher.stop_server().unwrap();
}

#[tracing::instrument(skip_all)]
fn update_segment_count_metric(metric: &Gauge, config: &config::Config) {
    debug!("Updating segment count metric");
    match std::fs::read_dir(&config.video_directory) {
        Ok(doot) => {
            let num = doot
                .filter_map(|i| i.ok())
                .map(|i| i.path())
                .filter(|i| {
                    if i.is_file() {
                        if let Some(ext) = i.extension() {
                            ext.to_str() == Some("ts")
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                })
                .count();
            metric.set(num as i64);
        }
        Err(e) => {
            warn!("Failed to read video directory, err={}", e);
        }
    }
}

#[tracing::instrument(skip_all)]
fn update_disk_usage_metric(metric: &Gauge, config: &config::Config) {
    debug!("Updating disk usage metric");
    match config.get_disk_usage() {
        Ok(disk_usage) => {
            metric.set(disk_usage.get_bytes() as i64);
        }
        Err(e) => {
            warn!("Failed to update disk usage, err={}", e);
        }
    }
}
