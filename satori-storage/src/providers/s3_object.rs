use crate::{EncryptionConfig, StorageProvider, StorageResult, encryption::KeyOperations};
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::StreamExt;
use object_store::{
    ObjectStore,
    aws::{AmazonS3, AmazonS3Builder},
    path::Path as ObjectPath,
};
use satori_common::Event;
use serde::Deserialize;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Deserialize)]
pub struct S3Config {
    bucket: String,
    region: String,
    endpoint: String,
    #[serde(default)]
    encryption: EncryptionConfig,
}

#[derive(Clone)]
pub struct S3Storage {
    store: Arc<AmazonS3>,
    encryption: EncryptionConfig,
}

impl S3Storage {
    pub fn new(config: S3Config) -> StorageResult<Self> {
        let store = AmazonS3Builder::from_env()
            .with_endpoint(&config.endpoint)
            .with_allow_http(true)
            .with_region(&config.region)
            .with_bucket_name(&config.bucket)
            .build()?;

        Ok(Self {
            store: Arc::new(store),
            encryption: config.encryption,
        })
    }

    fn get_event_path(&self, event: &Event) -> ObjectPath {
        ObjectPath::from(format!(
            "events/{}",
            event.metadata.get_filename().display()
        ))
    }

    fn get_event_path_from_filename(&self, filename: &Path) -> ObjectPath {
        ObjectPath::from(format!("events/{}", filename.display()))
    }

    fn get_segment_path(&self, camera_name: &str, filename: &Path) -> ObjectPath {
        ObjectPath::from(format!("segments/{}/{}", camera_name, filename.display()))
    }
}

#[async_trait]
impl StorageProvider for S3Storage {
    #[tracing::instrument(skip(self))]
    async fn put_event(&self, event: &Event) -> StorageResult<()> {
        let path = self.get_event_path(event);

        let data = serde_json::to_vec_pretty(&event)?;

        let info =
            crate::encryption::info::event_info_from_filename(&event.metadata.get_filename());
        let data = self.encryption.event.encrypt(info, data.into())?;

        self.store.put(&path, data.into()).await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list_events(&self) -> StorageResult<Vec<PathBuf>> {
        let prefix = ObjectPath::from("events");
        let mut list_stream = self.store.list(Some(&prefix));
        let mut results = Vec::new();

        while let Some(item) = list_stream.next().await {
            let meta = item?;
            if let Some(filename) = meta.location.filename()
                && filename.ends_with(".json")
            {
                results.push(PathBuf::from(filename));
            }
        }

        results.sort();
        Ok(results)
    }

    #[tracing::instrument(skip(self))]
    async fn get_event(&self, filename: &Path) -> StorageResult<Event> {
        let path = self.get_event_path_from_filename(filename);

        let get_result = self.store.get(&path).await?;
        let data = get_result.bytes().await?;

        let info = crate::encryption::info::event_info_from_filename(filename);
        let data = self.encryption.event.decrypt(info, data)?;

        Ok(serde_json::from_slice(&data)?)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_event(&self, event: &Event) -> StorageResult<()> {
        let path = self.get_event_path(event);
        self.store.delete(&path).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn delete_event_filename(&self, filename: &Path) -> StorageResult<()> {
        let path = self.get_event_path_from_filename(filename);
        self.store.delete(&path).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list_cameras(&self) -> StorageResult<Vec<String>> {
        let prefix = ObjectPath::from("segments");
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
    async fn put_segment(
        &self,
        camera_name: &str,
        filename: &Path,
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
    async fn list_segments(&self, camera_name: &str) -> StorageResult<Vec<PathBuf>> {
        let prefix = ObjectPath::from(format!("segments/{}", camera_name));
        let mut list_stream = self.store.list(Some(&prefix));
        let mut results = Vec::new();

        while let Some(item) = list_stream.next().await {
            let meta = item?;
            if let Some(filename) = meta.location.filename()
                && filename.ends_with(".ts")
            {
                results.push(PathBuf::from(filename));
            }
        }

        results.sort();
        Ok(results)
    }

    #[tracing::instrument(skip(self))]
    async fn get_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<Bytes> {
        let path = self.get_segment_path(camera_name, filename);

        let get_result = self.store.get(&path).await?;
        let data = get_result.bytes().await?;

        let info =
            crate::encryption::info::segment_info_from_camera_and_filename(camera_name, filename);
        let data = self.encryption.segment.decrypt(info, data)?;

        Ok(data)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<()> {
        let path = self.get_segment_path(camera_name, filename);
        self.store.delete(&path).await?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rand::Rng;
    use satori_testing_utils::MinioDriver;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    lazy_static::lazy_static! {
        static ref MINIO: Arc<Mutex<Option<MinioDriver>>> = Arc::new(Mutex::new(None));
    }

    #[ctor::ctor]
    fn init_minio() {
        let minio = MinioDriver::default();
        minio.set_credential_env_vars();
        MINIO.try_lock().unwrap().replace(minio);
    }

    #[ctor::dtor]
    fn cleanup_minio() {
        let minio = MINIO.try_lock().unwrap().take().unwrap();
        drop(minio);
    }

    fn generate_random_bucket_name() -> String {
        let id = rand::rng()
            .sample_iter(&rand::distr::Alphanumeric)
            .take(8)
            .map(char::from)
            .collect::<String>()
            .to_lowercase();

        format!("satori-storage-test-{id}")
    }

    mod no_encryption {
        use super::*;

        macro_rules! test {
            ( $test:ident ) => {
                #[tokio::test]
                async fn $test() {
                    let minio = MINIO.lock().await;
                    let minio = minio.as_ref().unwrap();

                    minio.wait_for_ready().await;

                    let bucket = super::generate_random_bucket_name();
                    minio.create_bucket(&bucket).await;

                    let provider = crate::StorageConfig::S3(S3Config {
                        bucket,
                        region: "".into(),
                        endpoint: minio.endpoint(),
                        encryption: EncryptionConfig::default(),
                    })
                    .create_provider()
                    .unwrap();

                    crate::providers::test::$test(provider).await;
                }
            };
        }

        crate::providers::test::all_storage_tests!(test);
    }

    mod encryption_hpke {
        use super::*;

        macro_rules! test {
            ( $test:ident ) => {
                #[tokio::test]
                async fn $test() {
                    let minio = MINIO.lock().await;
                    let minio = minio.as_ref().unwrap();

                    minio.wait_for_ready().await;

                    let bucket = super::generate_random_bucket_name();
                    minio.create_bucket(&bucket).await;

                    let provider = crate::StorageConfig::S3(S3Config {
                        bucket,
                        region: "".into(),
                        endpoint: minio.endpoint(),
                        encryption: toml::from_str(
                            "
[event]
kind = \"hpke\"
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEAZWyBUeaFatX3a3/OnqFljoEhAUHjrLgDJzzc5EqR/ho=
-----END PUBLIC KEY-----
\"\"\"
private_key = \"\"\"
-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VuBCIEIPAn/aQduWFV5VAlGQF79sBuzQItqFWu6FdJ4B77/UJ7
-----END PRIVATE KEY-----
\"\"\"
[segment]
kind = \"hpke\"
public_key = \"\"\"
-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEA4xQouJZhiNpBedFJBs3lE8FIOMQtnMzZG426m2nVjko=
-----END PUBLIC KEY-----
\"\"\"
private_key = \"\"\"
-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VuBCIEILhAcPMmERCi9QmBwH26wXzVo/6e5Lqw9lvA+8hf//xJ
-----END PRIVATE KEY-----
\"\"\"
",
                        )
                        .unwrap(),
                    })
                    .create_provider()
                    .unwrap();

                    crate::providers::test::$test(provider).await;
                }
            };
        }

        crate::providers::test::all_storage_tests!(test);
    }
}
