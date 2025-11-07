use metrics_exporter_prometheus::PrometheusBuilder;
use miette::{Context, IntoDiagnostic};
use std::net::SocketAddr;
use url::Url;

const TRIGGERS: &str = "satori_eventprocessor_triggers";
pub(crate) const ACTIVE_EVENTS: &str = "satori_eventprocessor_active_events";
const EXPIRED_EVENTS: &str = "satori_eventprocessor_expired_events";
const ARCHIVE_TASKS: &str = "satori_eventprocessor_archive_tasks";
pub(crate) const ARCHIVE_RETRY_QUEUE_LENGTH: &str =
    "satori_eventprocessor_archive_retry_queue_length";

pub(super) fn init(address: SocketAddr) -> miette::Result<()> {
    let builder = PrometheusBuilder::new();
    builder
        .with_http_listener(address)
        .install()
        .into_diagnostic()
        .wrap_err("Failed to start prometheus metrics exporter")?;

    metrics::describe_counter!(TRIGGERS, metrics::Unit::Count, "Trigger count");

    metrics::describe_gauge!(
        ACTIVE_EVENTS,
        metrics::Unit::Count,
        "Number of active events"
    );

    metrics::describe_counter!(EXPIRED_EVENTS, metrics::Unit::Count, "Expired events count");

    metrics::describe_counter!(
        ARCHIVE_TASKS,
        metrics::Unit::Count,
        "Processed archive task count"
    );

    metrics::describe_gauge!(
        ARCHIVE_RETRY_QUEUE_LENGTH,
        metrics::Unit::Count,
        "Number of tasks in archive retry queue"
    );

    Ok(())
}

pub(crate) fn inc_triggers_metric(trigger_id: String) {
    metrics::counter!(TRIGGERS, "id" => trigger_id).increment(1);
}

pub(crate) fn inc_expired_events_metric(trigger_id: String) {
    metrics::counter!(EXPIRED_EVENTS, "id" => trigger_id).increment(1);
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum ArchiveTaskResult {
    Success,
    Failure,
    FailureExpired,
}

pub(crate) fn inc_archive_task_metric(storage_api_url: &Url, result: ArchiveTaskResult) {
    let result = match result {
        ArchiveTaskResult::Success => "success",
        ArchiveTaskResult::Failure => "failure",
        ArchiveTaskResult::FailureExpired => "fail_expired",
    };

    metrics::counter!(ARCHIVE_TASKS, "storage" => storage_api_url.to_string(), "result" => result)
        .increment(1);
}
