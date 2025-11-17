use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::Provider;

/// Retrieve a specific video segment for a given camera.
#[derive(Debug, Clone, Parser)]
pub(crate) struct GetSegmentCommand {
    /// Name of the camera.
    camera: String,

    /// File to retrieve.
    file: String,
}

impl GetSegmentCommand {
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        let segment = storage
            .get_segment(&self.camera, &self.file)
            .await
            .into_diagnostic()?;
        println!("{segment:?}");
        Ok(())
    }
}
