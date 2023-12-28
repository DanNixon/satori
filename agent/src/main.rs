mod config;
mod ffmpeg;
mod jpeg_frame_decoder;
mod utils;

use axum::{
    http::header,
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use bytes::Bytes;
use clap::Parser;
use metrics_exporter_prometheus::PrometheusBuilder;
use std::{
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::TcpListener, task::JoinSet};
use tower_http::services::ServeDir;
use tracing::{debug, info, warn};

const METRIC_DISK_USAGE: &str = "satori_agent_disk_usage";
const METRIC_FFMPEG_INVOCATIONS: &str = "satori_agent_ffmpeg_invocations";
const METRIC_SEGMENTS: &str = "satori_agent_segments";

type SharedImageData = Arc<Mutex<Option<Bytes>>>;

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

    // Set up metrics server
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(cli.observability_address)
        .install()
        .expect("prometheus metrics exporter should be setup");

    metrics::describe_gauge!(
        METRIC_DISK_USAGE,
        metrics::Unit::Bytes,
        "Disk usage of this camera's output video directory"
    );

    metrics::describe_counter!(
        METRIC_FFMPEG_INVOCATIONS,
        metrics::Unit::Count,
        "Number of times ffmpeg has been invoked"
    );

    metrics::describe_gauge!(
        METRIC_SEGMENTS,
        metrics::Unit::Count,
        "Number of MPEG-TS segments generated"
    );

    // Create video output directory
    fs::create_dir_all(&config.video_directory).expect("should be able to create output directory");

    // Start streamer
    let mut streamer = ffmpeg::Streamer::new(config.clone());
    streamer.start().await;

    let mut tasks = JoinSet::<()>::new();

    // Configure HTTP server listener
    let listener = TcpListener::bind(&cli.http_server_address)
        .await
        .unwrap_or_else(|_| panic!("tcp listener should bind to {}", cli.http_server_address));

    let frame_image = SharedImageData::default();

    // Configure HTTP server routes
    let app = {
        let frame_image = frame_image.clone();

        Router::new()
            .route("/player", get(Html(include_str!("player.html"))))
            .route(
                "/frame.jpg",
                get(move || async move {
                    match frame_image.lock().unwrap().as_ref() {
                        Some(image) => {
                            ([(header::CONTENT_TYPE, "image/jpeg")], image.clone()).into_response()
                        }
                        None => axum::http::StatusCode::NOT_FOUND.into_response(),
                    }
                }),
            )
            .nest_service("/", ServeDir::new(config.video_directory.clone()))
    };

    // Start HTTP server
    info!("Starting HTTP server on {}", cli.http_server_address);
    tasks.spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut metrics_interval = tokio::time::interval(Duration::from_secs(30));
    let mut jpeg_rx = streamer.jpeg_subscribe();
    loop {
        tokio::select! {
            Ok(image) = jpeg_rx.recv() => {
                frame_image.lock().unwrap().replace(image);
            }
            _ = metrics_interval.tick() => {
                update_segment_count_metric(&config);
                update_disk_usage_metric(&config);
            }
            task = tasks.join_next() => {
                match task {
                    None => tokio::time::sleep(Duration::from_secs(5)).await,
                    Some(task) => {
                        info!("Task exits: {:?}", task);
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Exiting");
                break;
            }
        }
    }

    // Stop streamer
    streamer.stop().await;

    tasks.shutdown().await;
}

#[tracing::instrument(skip_all)]
fn update_segment_count_metric(config: &config::Config) {
    debug!("Updating segment count metric");

    match std::fs::read_dir(&config.video_directory) {
        Ok(contents) => {
            let ts_file_count = contents
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

            metrics::gauge!(METRIC_SEGMENTS, ts_file_count as f64);
        }
        Err(e) => {
            warn!("Failed to read video directory, err={}", e);
        }
    }
}

#[tracing::instrument(skip_all)]
fn update_disk_usage_metric(config: &config::Config) {
    debug!("Updating disk usage metric");

    match config.get_disk_usage() {
        Ok(disk_usage) => {
            metrics::gauge!(METRIC_DISK_USAGE, disk_usage.get_bytes() as f64);
        }
        Err(e) => {
            warn!("Failed to update disk usage, err={}", e);
        }
    }
}
