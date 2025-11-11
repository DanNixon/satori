use chrono::{Duration, Utc};
use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::{Provider, workflows};

/// Removes events matching specific rules.
#[derive(Debug, Clone, Parser)]
pub(crate) struct PruneEventsCommand {
    /// Number of days worth of events to keep
    #[arg(long)]
    days: i64,
}

impl PruneEventsCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        let time =
            Utc::now() - Duration::try_days(self.days).expect("days range should be within limits");
        workflows::prune_events_older_than(storage, time.into())
            .await
            .into_diagnostic()
    }
}
