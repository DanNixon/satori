use super::{CliResult, CliResultWithValue};
use clap::{Parser, Subcommand};
use satori_storage::{Provider, workflows};
use std::path::PathBuf;
use tracing::error;

/// Removes segments that are not referenced by any event.
#[derive(Debug, Clone, Parser)]
pub(crate) struct PruneSegmentsCommand {
    /// Number of parallel jobs to run in appropriate places in the selected workflow
    #[arg(short, long, default_value_t = 8)]
    jobs: usize,

    #[command(subcommand)]
    command: PruneSegmentsAction,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum PruneSegmentsAction {
    /// Calculate segments that are not referenced by any event and delete them
    Prune,

    /// Calculate segments that are not referenced by any event and produce a report detailing them
    Report {
        /// Filename of the report to create
        report: PathBuf,
    },

    /// Delete unreferenced events given a report
    Delete {
        /// Filename of the report to load
        report: PathBuf,
    },
}

impl PruneSegmentsCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        match &self.command {
            PruneSegmentsAction::Prune => {
                let unreferenced_segments =
                    calculate_unrefeferenced_segments(storage.clone(), self.jobs).await?;

                delete_unreferenced_segments(storage, unreferenced_segments, self.jobs).await
            }
            PruneSegmentsAction::Report { report } => {
                let unreferenced_segments =
                    calculate_unrefeferenced_segments(storage.clone(), self.jobs).await?;

                unreferenced_segments.save(report).map_err(|err| {
                    error!("{}", err);
                })
            }
            PruneSegmentsAction::Delete { report } => {
                let unreferenced_segments =
                    workflows::UnreferencedSegments::load(report).map_err(|err| {
                        error!("{}", err);
                    })?;

                delete_unreferenced_segments(storage, unreferenced_segments, self.jobs).await
            }
        }
    }
}

async fn calculate_unrefeferenced_segments(
    storage: Provider,
    jobs: usize,
) -> CliResultWithValue<workflows::UnreferencedSegments> {
    workflows::calculate_unreferenced_segments(storage, jobs)
        .await
        .map_err(|err| {
            error!("{}", err);
        })
}

async fn delete_unreferenced_segments(
    storage: Provider,
    segments: workflows::UnreferencedSegments,
    jobs: usize,
) -> CliResult {
    workflows::delete_unreferenced_segments(storage, segments, jobs)
        .await
        .map_err(|err| {
            error!("{}", err);
        })
}
