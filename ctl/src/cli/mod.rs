mod archive;
mod debug;
mod trigger;

use async_trait::async_trait;
use clap::{Parser, Subcommand};

pub(crate) type CliResult = Result<(), ()>;

#[async_trait]
pub(crate) trait CliExecute {
    async fn execute(&self) -> CliResult;
}

/// Control Satori NVR.
#[derive(Debug, Clone, Parser)]
#[command(author, version, about, long_about = None)]
pub(crate) struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[async_trait]
impl CliExecute for Cli {
    async fn execute(&self) -> CliResult {
        self.command.execute().await
    }
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum Command {
    Trigger(trigger::TriggerCommand),
    Archive(archive::ArchiveCommand),
    Debug(debug::DebugCommand),
}

#[async_trait]
impl CliExecute for Command {
    async fn execute(&self) -> CliResult {
        match self {
            Command::Trigger(cmd) => cmd.execute().await,
            Command::Archive(cmd) => cmd.execute().await,
            Command::Debug(cmd) => cmd.execute().await,
        }
    }
}
