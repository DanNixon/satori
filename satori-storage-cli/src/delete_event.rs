use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::Provider;
use std::path::PathBuf;

/// Delete a selection of event metadata files.
#[derive(Debug, Clone, Parser)]
pub(crate) struct DeleteEventCommand {
    /// Files to delete.
    file: Vec<PathBuf>,
}

impl DeleteEventCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        for path in &self.file {
            let event = storage.get_event(path).await.into_diagnostic()?;
            storage.delete_event(&event).await.into_diagnostic()?;
        }
        Ok(())
    }
}
