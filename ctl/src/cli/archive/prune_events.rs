use super::CliResult;
use clap::Parser;
use satori_storage::{workflows, Provider};
use tracing::error;

/// Removes events matching specific rules.
#[derive(Debug, Clone, Parser)]
pub(crate) struct PruneEventsCommand {
    /// Number of days worth of events to keep
    #[arg(long)]
    days: i64,
}

impl PruneEventsCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        let time = chrono::Utc::now() - chrono::Duration::days(self.days);
        workflows::prune_events_older_than(storage, time.into())
            .await
            .map_err(|err| {
                error!("{}", err);
            })
    }
}
