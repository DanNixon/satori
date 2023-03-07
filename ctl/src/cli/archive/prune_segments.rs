use super::CliResult;
use clap::Parser;
use satori_storage::{workflows, Provider};
use tracing::error;

/// Removes segments that are not referenced by any event.
#[derive(Debug, Clone, Parser)]
pub(crate) struct PruneSegmentsCommand {}

impl PruneSegmentsCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        workflows::prune_unreferenced_segments(storage)
            .await
            .map_err(|err| {
                error!("{}", err);
            })
    }
}
