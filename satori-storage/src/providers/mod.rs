pub mod dummy;
pub mod local;
pub mod s3_object;

#[cfg(test)]
mod test;

use super::{StorageProvider, StorageResult};
use async_trait::async_trait;
use bytes::Bytes;
use satori_common::Event;
use std::path::{Path, PathBuf};

#[derive(Clone)]
#[allow(clippy::large_enum_variant)]
pub enum Provider {
    Dummy(dummy::DummyStorage),
    Local(local::LocalStorage),
    S3(s3_object::S3Storage),
}

#[async_trait]
impl StorageProvider for Provider {
    async fn put_event(&self, event: &Event) -> StorageResult<()> {
        match self {
            Self::Dummy(p) => p.put_event(event).await,
            Self::Local(p) => p.put_event(event).await,
            Self::S3(p) => p.put_event(event).await,
        }
    }

    async fn list_events(&self) -> StorageResult<Vec<PathBuf>> {
        match self {
            Self::Dummy(p) => p.list_events().await,
            Self::Local(p) => p.list_events().await,
            Self::S3(p) => p.list_events().await,
        }
    }

    async fn get_event(&self, filename: &Path) -> StorageResult<Event> {
        match self {
            Self::Dummy(p) => p.get_event(filename).await,
            Self::Local(p) => p.get_event(filename).await,
            Self::S3(p) => p.get_event(filename).await,
        }
    }

    async fn delete_event(&self, event: &Event) -> StorageResult<()> {
        match self {
            Self::Dummy(p) => p.delete_event(event).await,
            Self::Local(p) => p.delete_event(event).await,
            Self::S3(p) => p.delete_event(event).await,
        }
    }

    async fn delete_event_filename(&self, filename: &Path) -> StorageResult<()> {
        match self {
            Self::Dummy(p) => p.delete_event_filename(filename).await,
            Self::Local(p) => p.delete_event_filename(filename).await,
            Self::S3(p) => p.delete_event_filename(filename).await,
        }
    }

    async fn list_cameras(&self) -> StorageResult<Vec<String>> {
        match self {
            Self::Dummy(p) => p.list_cameras().await,
            Self::Local(p) => p.list_cameras().await,
            Self::S3(p) => p.list_cameras().await,
        }
    }

    async fn put_segment(
        &self,
        camera_name: &str,
        filename: &Path,
        data: Bytes,
    ) -> StorageResult<()> {
        match self {
            Self::Dummy(p) => p.put_segment(camera_name, filename, data).await,
            Self::Local(p) => p.put_segment(camera_name, filename, data).await,
            Self::S3(p) => p.put_segment(camera_name, filename, data).await,
        }
    }

    async fn list_segments(&self, camera_name: &str) -> StorageResult<Vec<PathBuf>> {
        match self {
            Self::Dummy(p) => p.list_segments(camera_name).await,
            Self::Local(p) => p.list_segments(camera_name).await,
            Self::S3(p) => p.list_segments(camera_name).await,
        }
    }

    async fn get_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<Bytes> {
        match self {
            Self::Dummy(p) => p.get_segment(camera_name, filename).await,
            Self::Local(p) => p.get_segment(camera_name, filename).await,
            Self::S3(p) => p.get_segment(camera_name, filename).await,
        }
    }

    async fn delete_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<()> {
        match self {
            Self::Dummy(p) => p.delete_segment(camera_name, filename).await,
            Self::Local(p) => p.delete_segment(camera_name, filename).await,
            Self::S3(p) => p.delete_segment(camera_name, filename).await,
        }
    }
}
