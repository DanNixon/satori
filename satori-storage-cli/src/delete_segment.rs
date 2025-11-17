use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::Provider;

/// Delete a selection of video segment files for a given camera.
#[derive(Debug, Clone, Parser)]
pub(crate) struct DeleteSegmentCommand {
    /// Name of the camera.
    camera: String,

    /// Files to delete.
    file: Vec<String>,
}

impl DeleteSegmentCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        for path in &self.file {
            storage
                .delete_segment(&self.camera, path)
                .await
                .into_diagnostic()?;
        }
        Ok(())
    }
}
