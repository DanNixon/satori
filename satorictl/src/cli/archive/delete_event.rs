use super::CliResult;
use clap::Parser;
use satori_storage::{Provider, StorageProvider};
use std::path::PathBuf;
use tracing::error;

/// Delete a selection of event metadata files.
#[derive(Debug, Clone, Parser)]
pub(crate) struct DeleteEventCommand {
    /// Files to delete.
    file: Vec<PathBuf>,
}

impl DeleteEventCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        for path in &self.file {
            let event = storage.get_event(path).await.map_err(|err| {
                error!("{}", err);
            })?;
            storage.delete_event(&event).await.map_err(|err| {
                error!("{}", err);
            })?;
        }
        Ok(())
    }
}
