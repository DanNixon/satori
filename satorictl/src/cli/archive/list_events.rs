use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::{Provider, StorageProvider};

/// List all event metadata files.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ListEventsCommand {}

impl ListEventsCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        for event_file in storage.list_events().await.into_diagnostic()? {
            println!("{}", event_file.display());
        }
        Ok(())
    }
}
