use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::{Provider, StorageProvider};

/// List all cameras that have had segments stored.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ListCamerasCommand {}

impl ListCamerasCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        for camera in storage.list_cameras().await.into_diagnostic()? {
            println!("{camera}");
        }
        Ok(())
    }
}
