use super::CliResult;
use clap::Parser;
use satori_storage::{Provider, StorageProvider};
use tracing::error;

/// List all event metadata files.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ListEventsCommand {}

impl ListEventsCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        for event_file in storage.list_events().await.map_err(|err| {
            error!("{}", err);
        })? {
            println!("{}", event_file.display());
        }
        Ok(())
    }
}
