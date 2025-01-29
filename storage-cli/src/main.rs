mod cli;

use crate::cli::Cli;
use async_trait::async_trait;
use clap::Parser;

pub(crate) type CliResultWithValue<T> = Result<T, ()>;
pub(crate) type CliResult = CliResultWithValue<()>;

#[async_trait]
pub(crate) trait CliExecute {
    async fn execute(&self) -> CliResult;
}

#[tokio::main]
async fn main() -> CliResult {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();
    args.execute().await
}
