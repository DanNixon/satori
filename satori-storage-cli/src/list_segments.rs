use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::{Provider, StorageProvider};

/// List video segment files for a given camera.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ListSegmentsCommand {
    /// Name of the camera.
    camera: String,
}

impl ListSegmentsCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        for segment_file in storage
            .list_segments(&self.camera)
            .await
            .into_diagnostic()?
        {
            println!("{}", segment_file.display());
        }
        Ok(())
    }
}
