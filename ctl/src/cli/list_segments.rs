use super::CliResult;
use clap::Parser;
use satori_storage::{Provider, StorageProvider};
use tracing::error;

/// List video segment files for a given camera.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ListSegmentsCommand {
    /// Name of the camera.
    camera: String,
}

impl ListSegmentsCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        for segment_file in storage.list_segments(&self.camera).await.map_err(|err| {
            error!("{}", err);
        })? {
            println!("{}", segment_file.display());
        }
        Ok(())
    }
}
