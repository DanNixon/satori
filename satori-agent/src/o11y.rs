use crate::config::Config;
use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use std::{net::SocketAddr, path::Path};
use tracing::debug;

pub(crate) const METRIC_DISK_USAGE: &str = "satori_agent_disk_usage";
pub(crate) const METRIC_FFMPEG_INVOCATIONS: &str = "satori_agent_ffmpeg_invocations";
pub(crate) const METRIC_SEGMENTS: &str = "satori_agent_segments";
pub(crate) const METRIC_HTTP_REQUESTS: &str = "satori_agent_http_requests";

pub(super) fn init(address: SocketAddr) -> miette::Result<()> {
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(address)
        .install()
        .into_diagnostic()
        .wrap_err("Failed to start prometheus metrics exporter")?;

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

    metrics::describe_gauge!(
        METRIC_HTTP_REQUESTS,
        metrics::Unit::Count,
        "Number of requests to HTTP endpoints"
    );

    Ok(())
}

#[tracing::instrument(skip_all)]
pub(super) async fn update_segment_count_metric(playlist_filename: &Path) -> miette::Result<()> {
    debug!("Updating segment count metric");

    let playlist = crate::utils::load_playlist(playlist_filename).await?;
    metrics::gauge!(METRIC_SEGMENTS).set(playlist.segments.len() as f64);

    Ok(())
}

#[tracing::instrument(skip_all)]
pub(super) fn update_disk_usage_metric(config: &Config) -> miette::Result<()> {
    debug!("Updating disk usage metric");

    let disk_usage = config.get_disk_usage().into_diagnostic()?;
    metrics::gauge!(METRIC_DISK_USAGE).set(disk_usage as f64);

    Ok(())
}
