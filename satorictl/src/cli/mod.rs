mod archive;
mod debug;

use async_trait::async_trait;
use clap::{Parser, Subcommand};

#[async_trait]
pub(crate) trait CliExecute {
    async fn execute(&self) -> miette::Result<()>;
}

/// Control Satori NVR.
#[derive(Debug, Clone, Parser)]
#[command(
    author,
    version = satori_common::version!(),
)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[async_trait]
impl CliExecute for Cli {
    async fn execute(&self) -> miette::Result<()> {
        self.command.execute().await
    }
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Command {
    Archive(archive::ArchiveCommand),
    Debug(debug::DebugCommand),
}

#[async_trait]
impl CliExecute for Command {
    async fn execute(&self) -> miette::Result<()> {
        match self {
            Command::Archive(cmd) => cmd.execute().await,
            Command::Debug(cmd) => cmd.execute().await,
        }
    }
}
