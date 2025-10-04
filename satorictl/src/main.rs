mod cli;

use crate::cli::{Cli, CliExecute, CliResult};
use clap::Parser;

#[tokio::main]
async fn main() -> CliResult {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    args.execute().await
}
