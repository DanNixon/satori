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

use clap::Parser;
use clap::Subcommand;
use satori_storage::{Provider, StorageConfig};
use std::path::{Path, PathBuf};

/// Control Satori NVR storage.
#[derive(Debug, Clone, Parser)]
#[command(
    author,
    version = satori_common::version!(),
)]
pub(crate) struct Cli {
    /// Path to storage configuration.
    #[arg(long)]
    storage: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    let storage = create_provider(&args.storage).await?;
    args.command.execute(storage).await
}

async fn create_provider(path: &Path) -> miette::Result<Provider> {
    let storage_config: StorageConfig = satori_common::load_config_file(path)?;
    storage_config
        .create_provider()
        .map_err(|e| miette::miette!("Failed to create storage provider: {}", e))
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Command {
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

impl Command {
    async fn execute(&self, storage: Provider) -> miette::Result<()> {
        match &self {
            Command::ListEvents(cmd) => cmd.execute(storage).await,
            Command::ListCameras(cmd) => cmd.execute(storage).await,
            Command::ListSegments(cmd) => cmd.execute(storage).await,
            Command::GetEvent(cmd) => cmd.execute(storage).await,
            Command::GetSegment(cmd) => cmd.execute(storage).await,
            Command::DeleteEvent(cmd) => cmd.execute(storage).await,
            Command::DeleteSegment(cmd) => cmd.execute(storage).await,
            Command::PruneEvents(cmd) => cmd.execute(storage).await,
            Command::PruneSegments(cmd) => cmd.execute(storage).await,
            Command::ExportVideo(cmd) => cmd.execute(storage).await,
            Command::Explore(cmd) => cmd.execute(storage).await,
        }
    }
}
