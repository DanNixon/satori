use super::CliResult;
use clap::Parser;
use satori_storage::{workflows, Provider};
use std::{fs::File, io::Write, path::PathBuf};
use tracing::{error, info};

/// Exports a video file for a given event.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ExportVideoSubcommand {
    /// Filename of the event to export.
    event: PathBuf,

    /// Name of the camera who's video should be exported.
    camera: String,

    /// Name of the output video file.
    output: PathBuf,
}

impl ExportVideoSubcommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        info!("Creating output file: {}", self.output.display());
        let mut file = File::create(&self.output).map_err(|err| {
            error!("{}", err);
        })?;

        let file_content =
            workflows::export_event_video(storage, &self.event, Some(self.camera.clone()))
                .await
                .map_err(|err| {
                    error!("{}", err);
                })?;

        info!("Saving video: {}", self.output.display());
        file.write_all(&file_content).map_err(|err| {
            error!("{}", err);
        })?;

        Ok(())
    }
}
