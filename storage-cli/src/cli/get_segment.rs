use super::CliResult;
use clap::Parser;
use satori_storage::{Provider, StorageProvider};
use std::path::PathBuf;
use tracing::error;

/// Retrieve a specific video segment for a given camera.
#[derive(Debug, Clone, Parser)]
pub(crate) struct GetSegmentCommand {
    /// Name of the camera.
    camera: String,

    /// File to retrieve.
    file: PathBuf,
}

impl GetSegmentCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        let event = storage.get_segment(&self.camera, &self.file).await;
        println!(
            "{:?}",
            event.map_err(|err| {
                error!("{}", err);
            })?
        );
        Ok(())
    }
}
