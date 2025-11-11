mod cli;

use crate::cli::{Cli, CliExecute};
use clap::Parser;

#[tokio::main]
async fn main() -> miette::Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    args.execute().await
}
