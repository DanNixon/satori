use clap::Parser;
use miette::IntoDiagnostic;
use satori_storage::{EncryptionKey, KeyOperations};

/// Generate keys for encrypted storage.
///
/// Outputs a TOML formatted storage configuration for use in e.g. satori-archiver
/// and satori-storage-cli.
#[derive(Debug, Clone, Parser)]
pub(crate) struct GenerateKeyCommand {}

impl GenerateKeyCommand {
    pub(super) async fn execute(&self) -> miette::Result<()> {
        let key = EncryptionKey::Hpke(KeyOperations::generate());

        let repr = toml::to_string(&key).into_diagnostic()?;

        println!("{repr}");

        Ok(())
    }
}
