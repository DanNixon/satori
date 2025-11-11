use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::{Provider, StorageProvider};
use std::path::PathBuf;

/// Retrieve a specific video segment for a given camera.
#[derive(Debug, Clone, Parser)]
pub(crate) struct GetSegmentCommand {
    /// Name of the camera.
    camera: String,

    /// File to retrieve.
    file: PathBuf,
}

impl GetSegmentCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        let event = storage
            .get_segment(&self.camera, &self.file)
            .await
            .into_diagnostic()?;
        println!("{event:?}");
        Ok(())
    }
}
