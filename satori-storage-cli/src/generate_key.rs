use clap::Parser;
use satori_storage::{EncryptionKey, KeyOperations};

/// Generate keys for encrypted storage.
#[derive(Debug, Clone, Parser)]
pub(crate) struct GenerateKeyCommand {}

impl GenerateKeyCommand {
    pub(super) async fn execute(&self) -> miette::Result<()> {
        let key = EncryptionKey::generate();
        let key = key.as_bytes();
        println!("{key:?}");
        Ok(())
    }
}
