use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::Provider;
use std::path::PathBuf;

/// Retrieve metadata for a specific event.
#[derive(Debug, Clone, Parser)]
pub(crate) struct GetEventCommand {
    /// File to retrieve.
    file: PathBuf,
}

impl GetEventCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        let event = storage.get_event(&self.file).await.into_diagnostic()?;
        println!("{event:#?}");
        Ok(())
    }
}
