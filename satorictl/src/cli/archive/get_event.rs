use super::CliResult;
use clap::Parser;
use satori_storage::{Provider, StorageProvider};
use std::path::PathBuf;
use tracing::error;

/// Retrieve metadata for a specific event.
#[derive(Debug, Clone, Parser)]
pub(crate) struct GetEventCommand {
    /// File to retrieve.
    file: PathBuf,
}

impl GetEventCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        let event = storage.get_event(&self.file).await;
        println!(
            "{:#?}",
            event.map_err(|err| {
                error!("{}", err);
            })?
        );
        Ok(())
    }
}
