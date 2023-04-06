mod delete_event;
mod delete_segment;
mod explore;
mod export_video;
mod get_event;
mod get_segment;
mod list_cameras;
mod list_events;
mod list_segments;
mod prune_events;
mod prune_segments;

use super::{CliExecute, CliResult};
use async_trait::async_trait;
use clap::{Parser, Subcommand};
use satori_storage::StorageConfig;
use std::path::PathBuf;

/// Interact with an archive target.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ArchiveCommand {
    /// Path to storage configuration.
    #[arg(long)]
    storage: PathBuf,

    #[command(subcommand)]
    command: ArchiveSubcommand,
}

#[async_trait]
impl CliExecute for ArchiveCommand {
    async fn execute(&self) -> CliResult {
        let storage_config: StorageConfig = satori_common::load_config_file(&self.storage);
        let storage = storage_config.create_provider();

        match &self.command {
            ArchiveSubcommand::ListEvents(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::ListCameras(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::ListSegments(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::GetEvent(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::GetSegment(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::DeleteEvent(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::DeleteSegment(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::PruneEvents(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::PruneSegments(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::ExportVideo(cmd) => cmd.execute(storage).await,
            ArchiveSubcommand::Explore(cmd) => cmd.execute(storage).await,
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum ArchiveSubcommand {
    ListEvents(list_events::ListEventsCommand),
    ListCameras(list_cameras::ListCamerasCommand),
    ListSegments(list_segments::ListSegmentsCommand),
    GetEvent(get_event::GetEventCommand),
    GetSegment(get_segment::GetSegmentCommand),
    DeleteEvent(delete_event::DeleteEventCommand),
    DeleteSegment(delete_segment::DeleteSegmentCommand),
    PruneEvents(prune_events::PruneEventsCommand),
    PruneSegments(prune_segments::PruneSegmentsCommand),
    ExportVideo(export_video::ExportVideoSubcommand),
    Explore(explore::ExploreCommand),
}
