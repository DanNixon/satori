mod delete_event;
mod delete_segment;
mod explore;
mod export_video;
mod generate_key;
mod get_event;
mod get_segment;
mod list_cameras;
mod list_events;
mod list_segments;
mod prune_events;
mod prune_segments;

use clap::Parser;
use clap::Subcommand;
use satori_storage::Provider;
use satori_storage::StorageConfig;
use std::path::{Path, PathBuf};

/// Control Satori NVR storage.
#[derive(Debug, Clone, Parser)]
#[command(
    author,
    version = satori_common::version!(),
)]
pub(crate) struct Cli {
    /// Path to storage configuration.
    #[arg(long, global = true)]
    storage: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    args.command.execute(args.storage.as_deref()).await
}

async fn create_provider(path: &Path) -> miette::Result<Provider> {
    let storage_config: StorageConfig = satori_common::load_config_file(path)?;
    storage_config
        .try_into()
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
    GenerateKey(generate_key::GenerateKeyCommand),
}

impl Command {
    async fn execute(&self, storage_path: Option<&Path>) -> miette::Result<()> {
        match &self {
            Command::GenerateKey(cmd) => cmd.execute().await,
            _ => {
                let storage_path = storage_path.ok_or_else(|| {
                    miette::miette!("--storage argument is required for this command")
                })?;
                let storage = create_provider(storage_path).await?;
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
                    Command::GenerateKey(_) => unreachable!(),
                }
            }
        }
    }
}
