use super::CliResult;
use clap::Parser;
use satori_storage::{Provider, workflows};
use std::{fs::File, io::Write, path::PathBuf};
use tracing::{error, info};

/// Exports a video file for a given event.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ExportVideoSubcommand {
    /// Name of the camera who's video should be exported.
    ///
    /// Can be omitted for events containing a single camera.
    #[arg(short, long)]
    camera: Option<String>,

    /// Name of the output video file.
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Filename of the event to export.
    event: PathBuf,
}

impl ExportVideoSubcommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        let (event, file_content) =
            workflows::export_event_video(storage, &self.event, self.camera.clone())
                .await
                .map_err(|err| {
                    error!("{}", err);
                })?;

        // Use the user provided output filename if one exists, otherwise generate one.
        let output_filename = match &self.output {
            Some(filename) => filename.clone(),
            None => {
                workflows::generate_video_filename(&event, self.camera.clone()).map_err(|err| {
                    error!("{}", err);
                })?
            }
        };

        info!("Saving video: {}", output_filename.display());
        let mut file = File::create(&output_filename).map_err(|err| {
            error!("{}", err);
        })?;
        file.write_all(&file_content).map_err(|err| {
            error!("{}", err);
        })?;

        Ok(())
    }
}
