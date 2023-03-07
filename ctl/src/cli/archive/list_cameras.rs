use super::CliResult;
use clap::Parser;
use satori_storage::{Provider, StorageProvider};
use tracing::error;

/// List all cameras that have had segments stored.
#[derive(Debug, Clone, Parser)]
pub(crate) struct ListCamerasCommand {}

impl ListCamerasCommand {
    pub(super) async fn execute(&self, storage: Provider) -> CliResult {
        for camera in storage.list_cameras().await.map_err(|err| {
            error!("{}", err);
        })? {
            println!("{camera}");
        }
        Ok(())
    }
}
