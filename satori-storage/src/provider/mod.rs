#[cfg(test)]
mod test;

use super::StorageResult;
use crate::{EncryptionConfig, StorageConfig, encryption::KeyOperations};
use bytes::Bytes;
use futures::StreamExt;
use object_store::{
    ObjectStore, ObjectStoreScheme, aws::AmazonS3Builder, local::LocalFileSystem, memory::InMemory,
    path::Path,
};
use satori_common::Event;
use std::sync::Arc;
use tracing::info;
use url::Url;

#[derive(Clone)]
pub struct Provider {
    store: Arc<dyn ObjectStore>,
    encryption: EncryptionConfig,
}

impl TryFrom<StorageConfig> for Provider {
    type Error = crate::StorageError;

    fn try_from(value: StorageConfig) -> Result<Self, Self::Error> {
        Self::new(value.url, value.encryption)
    }
}

impl Provider {
    pub fn new(url: Url, encryption: EncryptionConfig) -> StorageResult<Self> {
        let store = backend_from_url(&url)?;
        Ok(Self { store, encryption })
    }

    fn get_event_path(&self, event: &Event) -> Path {
        Path::from(format!(
            "events/{}",
            event.metadata.get_filename().display()
        ))
    }

    fn get_event_path_from_filename(&self, filename: &std::path::Path) -> Path {
        Path::from(format!("events/{}", filename.display()))
    }

    fn get_segment_path(&self, camera_name: &str, filename: &std::path::Path) -> Path {
        Path::from(format!("segments/{}/{}", camera_name, filename.display()))
    }
}

pub fn backend_from_url(url: &Url) -> StorageResult<Arc<dyn ObjectStore>> {
    let (scheme, path) = ObjectStoreScheme::parse(url).unwrap();

    let store: Arc<dyn ObjectStore> = match scheme {
        ObjectStoreScheme::Memory => {
            info!("Creating in memory backend from URL {url}");
            Arc::new(InMemory::new())
        }
        ObjectStoreScheme::Local => {
            info!("Creating local filesystem backend from URL {url}");

            // Make prefix path absolute. Leading `/` is stripped in parsing, but required for
            // `LocalFileSystem`.
            let path = std::path::PathBuf::from("/").join(path.as_ref());

            Arc::new(LocalFileSystem::new_with_prefix(path)?.with_automatic_cleanup(true))
        }
        ObjectStoreScheme::AmazonS3 => {
            info!("Creating S3 backend from URL {url}");
            Arc::new(
                AmazonS3Builder::from_env()
                    .with_url(url.to_string())
                    .build()?,
            )
        }
        _ => {
            return Err(crate::StorageError::NoBackendForUrl(url.to_owned()));
        }
    };

    Ok(store)
}

impl Provider {
    #[tracing::instrument(skip(self))]
    pub async fn put_event(&self, event: &Event) -> StorageResult<()> {
        let path = self.get_event_path(event);

        let data = serde_json::to_vec_pretty(&event)?;

        let info =
            crate::encryption::info::event_info_from_filename(&event.metadata.get_filename());
        let data = self.encryption.event.encrypt(info, data.into())?;

        self.store.put(&path, data.into()).await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_events(&self) -> StorageResult<Vec<std::path::PathBuf>> {
        let prefix = Path::from("events");
        let mut list_stream = self.store.list(Some(&prefix));
        let mut results = Vec::new();

        while let Some(item) = list_stream.next().await {
            let meta = item?;
            if let Some(filename) = meta.location.filename()
                && filename.ends_with(".json")
            {
                results.push(std::path::PathBuf::from(filename));
            }
        }

        results.sort();
        Ok(results)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_event(&self, filename: &std::path::Path) -> StorageResult<Event> {
        let path = self.get_event_path_from_filename(filename);

        let get_result = self.store.get(&path).await?;
        let data = get_result.bytes().await?;

        let info = crate::encryption::info::event_info_from_filename(filename);
        let data = self.encryption.event.decrypt(info, data)?;

        Ok(serde_json::from_slice(&data)?)
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete_event(&self, event: &Event) -> StorageResult<()> {
        let path = self.get_event_path(event);
        self.store.delete(&path).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete_event_filename(&self, filename: &std::path::Path) -> StorageResult<()> {
        let path = self.get_event_path_from_filename(filename);
        self.store.delete(&path).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_cameras(&self) -> StorageResult<Vec<String>> {
        let prefix = Path::from("segments");
        let list_result = self.store.list_with_delimiter(Some(&prefix)).await?;

        let mut cameras: Vec<String> = list_result
            .common_prefixes
            .into_iter()
            .filter_map(|p| p.filename().map(|s| s.to_string()))
            .collect();

        cameras.sort();
        Ok(cameras)
    }

    #[tracing::instrument(skip(self, data))]
    pub async fn put_segment(
        &self,
        camera_name: &str,
        filename: &std::path::Path,
        data: Bytes,
    ) -> StorageResult<()> {
        let path = self.get_segment_path(camera_name, filename);

        let info =
            crate::encryption::info::segment_info_from_camera_and_filename(camera_name, filename);
        let data = self.encryption.segment.encrypt(info, data)?;

        self.store.put(&path, data.into()).await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn list_segments(&self, camera_name: &str) -> StorageResult<Vec<std::path::PathBuf>> {
        let prefix = Path::from(format!("segments/{}", camera_name));
        let mut list_stream = self.store.list(Some(&prefix));
        let mut results = Vec::new();

        while let Some(item) = list_stream.next().await {
            let meta = item?;
            if let Some(filename) = meta.location.filename()
                && filename.ends_with(".ts")
            {
                results.push(std::path::PathBuf::from(filename));
            }
        }

        results.sort();
        Ok(results)
    }

    #[tracing::instrument(skip(self))]
    pub async fn get_segment(
        &self,
        camera_name: &str,
        filename: &std::path::Path,
    ) -> StorageResult<Bytes> {
        let path = self.get_segment_path(camera_name, filename);

        let get_result = self.store.get(&path).await?;
        let data = get_result.bytes().await?;

        let info =
            crate::encryption::info::segment_info_from_camera_and_filename(camera_name, filename);
        let data = self.encryption.segment.decrypt(info, data)?;

        Ok(data)
    }

    #[tracing::instrument(skip(self))]
    pub async fn delete_segment(
        &self,
        camera_name: &str,
        filename: &std::path::Path,
    ) -> StorageResult<()> {
        let path = self.get_segment_path(camera_name, filename);
        self.store.delete(&path).await?;
        Ok(())
    }
}
