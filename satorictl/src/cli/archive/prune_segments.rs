use clap::{Parser, Subcommand};
use miette::IntoDiagnostic;
use satori_storage::{Provider, workflows};
use std::path::PathBuf;

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
    pub(super) async fn execute(&self, storage: Provider) -> miette::Result<()> {
        match &self.command {
            PruneSegmentsAction::Prune => {
                let unreferenced_segments =
                    workflows::calculate_unreferenced_segments(storage.clone(), self.jobs)
                        .await
                        .into_diagnostic()?;

                workflows::delete_unreferenced_segments(storage, unreferenced_segments, self.jobs)
                    .await
                    .into_diagnostic()
            }
            PruneSegmentsAction::Report { report } => {
                let unreferenced_segments =
                    workflows::calculate_unreferenced_segments(storage.clone(), self.jobs)
                        .await
                        .into_diagnostic()?;

                unreferenced_segments.save(report).into_diagnostic()
            }
            PruneSegmentsAction::Delete { report } => {
                let unreferenced_segments =
                    workflows::UnreferencedSegments::load(report).into_diagnostic()?;

                workflows::delete_unreferenced_segments(storage, unreferenced_segments, self.jobs)
                    .await
                    .into_diagnostic()
            }
        }
    }
}
