mod config;
mod ffmpeg;
mod jpeg_frame_decoder;
mod o11y;
mod utils;

use axum::{
    Router,
    body::Body,
    extract::{Query, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use bytes::{BufMut, Bytes};
use chrono::{DateTime, FixedOffset, Utc};
use clap::Parser;
use miette::{Context, IntoDiagnostic};
use serde::Deserialize;
use std::{
    fs,
    net::SocketAddr,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::net::TcpListener;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};

type SharedImageData = Arc<Mutex<Option<Bytes>>>;

/// Run the camera agent.
///
/// Handles restreaming a single camera as HLS with history.
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

#[derive(Clone)]
struct AppContext {
    frame_image: Arc<Mutex<Option<Bytes>>>,
    jpeg_multipart_tx: tokio::sync::broadcast::Sender<Bytes>,
    playlist_filename: PathBuf,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config: config::Config = satori_common::load_config_file(&cli.config)?;

    info!("FFmpeg version: {}", ffmpeg::get_ffmpeg_version());

    o11y::init(cli.observability_address)?;

    // Create video output directory
    fs::create_dir_all(&config.video_directory)
        .into_diagnostic()
        .wrap_err("Failed to create output directory")?;

    // Channel for JPEG frames
    let (jpeg_tx, mut jpeg_rx) = tokio::sync::broadcast::channel(8);

    // Start streamer
    let mut streamer = ffmpeg::Streamer::new(config.clone(), jpeg_tx);
    let playlist_filename = streamer.playlist_filename();
    streamer.start().await;

    // Configure HTTP server listener
    let listener = TcpListener::bind(&cli.http_server_address)
        .await
        .into_diagnostic()
        .wrap_err(format!(
            "Failed to bind API server to {}",
            cli.http_server_address
        ))?;

    // Configure HTTP server endpoints
    let frame_image = SharedImageData::default();
    let (jpeg_multipart_tx, _) = tokio::sync::broadcast::channel::<Bytes>(8);

    let context = AppContext {
        frame_image: frame_image.clone(),
        jpeg_multipart_tx: jpeg_multipart_tx.clone(),
        playlist_filename: playlist_filename.clone(),
    };

    let app = Router::new()
        .route("/player", get(player_handler))
        .route("/jpeg", get(jpeg_handler))
        .route("/mjpeg", get(mjpeg_handler))
        .route("/hls", get(hls_handler))
        .nest_service("/hls/", ServeDir::new(config.video_directory.clone()))
        .with_state(context);

    // Start HTTP server
    info!("Starting HTTP server on {}", cli.http_server_address);
    let server_handle = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let mut metrics_interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        tokio::select! {
            Ok(image) = jpeg_rx.recv() => {
                let mut body = bytes::BytesMut::new();
                body.put_slice(b"--frame\r\n");
                body.put_slice(format!("{}: image/jpeg\r\n", header::CONTENT_TYPE).as_bytes());
                body.put_slice(format!("{}: {}\r\n", header::CONTENT_LENGTH, image.len()).as_bytes());
                body.put_slice(b"\r\n");
                body.put_slice(&image);
                let _ = jpeg_multipart_tx.send(body.into());

                frame_image.lock().unwrap().replace(image);
            }
            _ = metrics_interval.tick() => {
                if let Err(e) = o11y::update_segment_count_metric(&playlist_filename).await {
                    warn!("Failed to update segment count metric: {e}");
                }

                if let Err(e) = o11y::update_disk_usage_metric(&config) {
                    warn!("Failed to update disk usage metric: {e}");
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

    // Stop server
    info!("Stopping HTTP server");
    server_handle.abort();
    let _ = server_handle.await;

    Ok(())
}

async fn player_handler(State(_ctx): State<AppContext>) -> Response {
    metrics::counter!(o11y::METRIC_HTTP_REQUESTS, "path" => "/player").increment(1);

    Html(include_str!("player.html")).into_response()
}

async fn jpeg_handler(State(ctx): State<AppContext>) -> Response {
    metrics::counter!(o11y::METRIC_HTTP_REQUESTS, "path" => "/jpeg").increment(1);

    match ctx.frame_image.lock().unwrap().as_ref() {
        Some(image) => ([(header::CONTENT_TYPE, "image/jpeg")], image.clone()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn mjpeg_handler(State(ctx): State<AppContext>) -> Response {
    metrics::counter!(o11y::METRIC_HTTP_REQUESTS, "path" => "/mjpeg").increment(1);

    let stream = BroadcastStream::new(ctx.jpeg_multipart_tx.subscribe());
    let body = Body::from_stream(stream);

    let response = (
        [(
            header::CONTENT_TYPE,
            "multipart/x-mixed-replace; boundary=frame",
        )],
        body,
    );
    response.into_response()
}

#[derive(Debug, Deserialize)]
struct HlsQueryParams {
    /// Start timestamp in RFC3339 format
    since: Option<String>,
    /// End timestamp in RFC3339 format
    until: Option<String>,
    /// Duration in the past (e.g., "10s", "5m", "1h")
    last: Option<String>,
}

async fn hls_handler(
    State(ctx): State<AppContext>,
    Query(params): Query<HlsQueryParams>,
) -> Response {
    metrics::counter!(o11y::METRIC_HTTP_REQUESTS, "path" => "/hls").increment(1);

    let f = async || -> miette::Result<Response> {
        // Validate that 'last' is not used with 'since' or 'until'
        if params.last.is_some() && (params.since.is_some() || params.until.is_some()) {
            return Err(miette::miette!(
                "The 'last' parameter cannot be used with 'since' or 'until'"
            ));
        }

        let mut playlist = utils::load_playlist(&ctx.playlist_filename).await?;

        // Determine time range for filtering
        let (start, end) = if let Some(last_duration_str) = &params.last {
            // Parse the duration
            let duration = humantime::parse_duration(last_duration_str)
                .into_diagnostic()
                .wrap_err("Failed to parse 'last' duration")?;

            let now = Utc::now();
            let start_time = now - chrono::Duration::from_std(duration).into_diagnostic()?;

            // Convert to FixedOffset
            let start: DateTime<FixedOffset> = start_time.into();
            (Some(start), None)
        } else {
            // Parse 'since' and 'until' if provided
            let start = if let Some(since_str) = &params.since {
                Some(
                    DateTime::parse_from_rfc3339(since_str)
                        .into_diagnostic()
                        .wrap_err("Failed to parse 'since' timestamp")?,
                )
            } else {
                None
            };

            let end = if let Some(until_str) = &params.until {
                Some(
                    DateTime::parse_from_rfc3339(until_str)
                        .into_diagnostic()
                        .wrap_err("Failed to parse 'until' timestamp")?,
                )
            } else {
                None
            };

            (start, end)
        };

        // Apply time filtering
        playlist = satori_common::filter_playlist_by_time(playlist, start, end)?;

        // Prefix "hls/" to all paths in playlist
        for segment in &mut playlist.segments {
            if !segment.uri.starts_with("hls/") {
                segment.uri = format!("hls/{}", segment.uri);
            }
        }

        let mut s = Vec::new();
        playlist.write_to(&mut s).into_diagnostic()?;

        let response = ([(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")], s);
        Ok(response.into_response())
    };

    f().await.unwrap_or_else(|e| {
        error!("{e}");
        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hls_query_params_deserialization() {
        // Test with 'since' parameter
        let query = "since=2022-12-30T18:10:00%2B00:00";
        let params: HlsQueryParams = serde_urlencoded::from_str(query).unwrap();
        assert!(params.since.is_some());
        assert!(params.until.is_none());
        assert!(params.last.is_none());

        // Test with 'until' parameter
        let query = "until=2022-12-30T18:10:00%2B00:00";
        let params: HlsQueryParams = serde_urlencoded::from_str(query).unwrap();
        assert!(params.since.is_none());
        assert!(params.until.is_some());
        assert!(params.last.is_none());

        // Test with 'last' parameter
        let query = "last=5m";
        let params: HlsQueryParams = serde_urlencoded::from_str(query).unwrap();
        assert!(params.since.is_none());
        assert!(params.until.is_none());
        assert!(params.last.is_some());
        assert_eq!(params.last.unwrap(), "5m");

        // Test with both 'since' and 'until'
        let query = "since=2022-12-30T18:10:00%2B00:00&until=2022-12-30T18:20:00%2B00:00";
        let params: HlsQueryParams = serde_urlencoded::from_str(query).unwrap();
        assert!(params.since.is_some());
        assert!(params.until.is_some());
        assert!(params.last.is_none());

        // Test with no parameters
        let query = "";
        let params: HlsQueryParams = serde_urlencoded::from_str(query).unwrap();
        assert!(params.since.is_none());
        assert!(params.until.is_none());
        assert!(params.last.is_none());
    }
}
