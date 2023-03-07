use crate::{StorageError, StorageProvider, StorageResult};
use async_trait::async_trait;
use bytes::Bytes;
use s3::{creds::Credentials, region::Region, Bucket};
use satori_common::Event;
use serde::Deserialize;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

#[derive(Debug, Deserialize)]
pub struct S3Config {
    bucket: String,
    region: String,
    endpoint: String,
}

#[derive(Clone)]
pub struct S3Storage {
    bucket: Bucket,
}

impl S3Storage {
    pub fn new(config: S3Config) -> Self {
        let bucket = Bucket::new(
            &config.bucket,
            Region::Custom {
                region: config.region,
                endpoint: config.endpoint,
            },
            Credentials::default().unwrap(),
        )
        .unwrap()
        .with_path_style();

        Self { bucket }
    }

    fn get_events_path(&self) -> PathBuf {
        PathBuf::from("events")
    }

    fn get_event_filename(&self, event: &Event) -> PathBuf {
        self.get_events_path().join(event.metadata.get_filename())
    }

    fn get_segments_root_path(&self) -> PathBuf {
        PathBuf::from("segments/")
    }

    fn get_segments_path(&self, camera_name: &str) -> PathBuf {
        self.get_segments_root_path().join(camera_name)
    }

    fn get_segment_filename(&self, camera_name: &str, filename: &Path) -> PathBuf {
        self.get_segments_path(camera_name).join(filename)
    }

    #[tracing::instrument(skip(self))]
    async fn list_path(&self, path: &Path) -> StorageResult<Vec<PathBuf>> {
        let response = self
            .bucket
            .list(path.to_str().unwrap().into(), None)
            .await?;

        Ok(response
            .into_iter()
            .flat_map(|i| {
                i.contents
                    .into_iter()
                    .map(|i| PathBuf::from(i.key))
                    .collect::<Vec<PathBuf>>()
            })
            .collect())
    }

    #[tracing::instrument(skip(self))]
    async fn delete_path(&self, path: &Path) -> StorageResult<()> {
        let status_code = self
            .bucket
            .delete_object(path.to_str().unwrap())
            .await?
            .status_code();

        if status_code == 204 {
            Ok(())
        } else {
            Err(StorageError::S3Failure(status_code))
        }
    }
}

#[async_trait]
impl StorageProvider for S3Storage {
    #[tracing::instrument(skip(self))]
    async fn put_event(&self, event: &Event) -> StorageResult<()> {
        let path = self.get_event_filename(event);

        let data = serde_json::to_vec_pretty(&event)?;

        let status_code = self
            .bucket
            .put_object(path.to_str().unwrap(), &data)
            .await?
            .status_code();

        if status_code == 200 {
            Ok(())
        } else {
            Err(StorageError::S3Failure(status_code))
        }
    }

    #[tracing::instrument(skip(self))]
    async fn list_events(&self) -> StorageResult<Vec<PathBuf>> {
        Ok(self
            .list_path(&self.get_events_path())
            .await?
            .into_iter()
            .map(|p| PathBuf::from(p.file_name().unwrap().to_str().unwrap()))
            .collect())
    }

    #[tracing::instrument(skip(self))]
    async fn get_event(&self, filename: &Path) -> StorageResult<Event> {
        let path = self.get_events_path().join(filename);

        let response = self.bucket.get_object(path.to_str().unwrap()).await?;

        if response.status_code() == 200 {
            Ok(serde_json::from_slice(response.bytes())?)
        } else {
            Err(StorageError::S3Failure(response.status_code()))
        }
    }

    #[tracing::instrument(skip(self))]
    async fn delete_event(&self, event: &Event) -> StorageResult<()> {
        self.delete_path(&self.get_event_filename(event)).await
    }

    #[tracing::instrument(skip(self))]
    async fn delete_event_filename(&self, filename: &Path) -> StorageResult<()> {
        self.delete_path(&self.get_events_path().join(filename))
            .await
    }

    #[tracing::instrument(skip(self))]
    async fn list_cameras(&self) -> StorageResult<Vec<String>> {
        let mut cameras = HashSet::new();

        for path in self.list_path(&self.get_segments_root_path()).await? {
            let comps: Vec<std::path::Component> = path.components().collect();

            if let std::path::Component::Normal(camera_name) = comps[1] {
                cameras.insert(camera_name.to_str().unwrap().to_owned());
            }
        }

        let mut cameras: Vec<String> = cameras.drain().collect();
        cameras.sort();

        Ok(cameras)
    }

    #[tracing::instrument(skip(self, data))]
    async fn put_segment(
        &self,
        camera_name: &str,
        filename: &Path,
        data: Bytes,
    ) -> StorageResult<()> {
        let path = self.get_segment_filename(camera_name, filename);

        let status_code = self
            .bucket
            .put_object(path.to_str().unwrap(), &data)
            .await?
            .status_code();

        if status_code == 200 {
            Ok(())
        } else {
            Err(StorageError::S3Failure(status_code))
        }
    }

    #[tracing::instrument(skip(self))]
    async fn list_segments(&self, camera_name: &str) -> StorageResult<Vec<PathBuf>> {
        Ok(self
            .list_path(&self.get_segments_path(camera_name))
            .await?
            .into_iter()
            .map(|p| PathBuf::from(p.file_name().unwrap().to_str().unwrap()))
            .collect())
    }

    #[tracing::instrument(skip(self))]
    async fn get_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<Bytes> {
        let path = self.get_segment_filename(camera_name, filename);

        let response = self.bucket.get_object(path.to_str().unwrap()).await?;

        if response.status_code() == 200 {
            Ok(Bytes::from(response.bytes().to_owned()))
        } else {
            Err(StorageError::S3Failure(response.status_code()))
        }
    }

    #[tracing::instrument(skip(self))]
    async fn delete_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<()> {
        self.delete_path(&self.get_segment_filename(camera_name, filename))
            .await
    }
}

#[cfg(test)]
mod test {
    use super::super::test as storage_tests;
    use super::*;
    use crate::Provider;
    use rand::Rng;
    use s3::BucketConfiguration;

    async fn run_test<Fut>(test_func: impl FnOnce(Provider) -> Fut)
    where
        Fut: std::future::Future<Output = ()>,
    {
        let id = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect::<String>()
            .to_lowercase();
        let bucket_name = format!("satori-storage-test-{id}");

        Bucket::create_with_path_style(
            &bucket_name,
            Region::Custom {
                region: "".into(),
                endpoint: "http://localhost:9000".into(),
            },
            Credentials::default().unwrap(),
            BucketConfiguration::default(),
        )
        .await
        .unwrap();

        let provider = super::super::super::StorageConfig::S3(S3Config {
            bucket: bucket_name,
            region: "".into(),
            endpoint: "http://localhost:9000".into(),
        })
        .create_provider();

        test_func(provider).await;
    }

    #[tokio::test]
    async fn test_init() {
        run_test(storage_tests::test_init).await;
    }

    #[tokio::test]
    async fn test_add_first_event() {
        run_test(storage_tests::test_add_first_event).await;
    }

    #[tokio::test]
    async fn test_add_event() {
        run_test(storage_tests::test_add_event).await;
    }

    #[tokio::test]
    async fn test_add_segment_new_camera() {
        run_test(storage_tests::test_add_segment_new_camera).await;
    }

    #[tokio::test]
    async fn test_add_segment_existing_camera() {
        run_test(storage_tests::test_add_segment_existing_camera).await;
    }

    #[tokio::test]
    async fn test_event_getters() {
        run_test(storage_tests::test_event_getters).await;
    }

    #[tokio::test]
    async fn test_segment_getters() {
        run_test(storage_tests::test_segment_getters).await;
    }

    #[tokio::test]
    async fn test_delete_event() {
        run_test(storage_tests::test_delete_event).await;
    }

    #[tokio::test]
    async fn test_delete_event_filename() {
        run_test(storage_tests::test_delete_event_filename).await;
    }

    #[tokio::test]
    async fn test_delete_segment() {
        run_test(storage_tests::test_delete_segment).await;
    }

    #[tokio::test]
    async fn test_delete_last_segment_deletes_camera() {
        run_test(storage_tests::test_delete_last_segment_deletes_camera).await;
    }
}
