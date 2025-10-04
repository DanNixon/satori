use super::CliResult;
use clap::Parser;
use satori_storage::{Provider, StorageProvider};
use std::path::PathBuf;
use tracing::error;

/// Delete a selection of video segment files for a given camera.
#[derive(Debug, Clone, Parser)]
pub(crate) struct DeleteSegmentCommand {
    /// Name of the camera.
    camera: String,

    /// Files to delete.
    file: Vec<PathBuf>,
}

impl DeleteSegmentCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        for path in &self.file {
            storage
                .delete_segment(&self.camera, path)
                .await
                .map_err(|err| {
                    error!("{}", err);
                })?;
        }
        Ok(())
    }
}
