mod encryption;
pub use self::encryption::{EncryptionConfig, EncryptionKey};

pub mod error;
pub use self::error::{StorageError, StorageResult};

mod providers;
pub use self::providers::Provider;

pub mod workflows;

use async_trait::async_trait;
use bytes::Bytes;
use satori_common::Event;
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StorageConfig {
    Dummy(providers::dummy::DummyConfig),
    Local(providers::local::LocalConfig),
    S3(providers::s3_object::S3Config),
}

impl StorageConfig {
    pub fn create_provider(self) -> Provider {
        match self {
            Self::Dummy(config) => Provider::Dummy(providers::dummy::DummyStorage::new(config)),
            Self::Local(config) => Provider::Local(providers::local::LocalStorage::new(config)),
            Self::S3(config) => Provider::S3(providers::s3_object::S3Storage::new(config)),
        }
    }
}

#[async_trait]
pub trait StorageProvider {
    async fn put_event(&self, event: &Event) -> StorageResult<()>;
    async fn list_events(&self) -> StorageResult<Vec<PathBuf>>;
    async fn get_event(&self, filename: &Path) -> StorageResult<Event>;
    async fn delete_event(&self, event: &Event) -> StorageResult<()>;
    async fn delete_event_filename(&self, filename: &Path) -> StorageResult<()>;

    async fn list_cameras(&self) -> StorageResult<Vec<String>>;

    async fn put_segment(
        &self,
        camera_name: &str,
        filename: &Path,
        data: Bytes,
    ) -> StorageResult<()>;
    async fn list_segments(&self, camera_name: &str) -> StorageResult<Vec<PathBuf>>;
    async fn get_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<Bytes>;
    async fn delete_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<()>;
}
