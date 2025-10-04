use crate::{
    EncryptionConfig, StorageError, StorageProvider, StorageResult, encryption::KeyOperations,
};
use async_trait::async_trait;
use bytes::Bytes;
use s3::{Bucket, creds::Credentials, region::Region};
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
    #[serde(default)]
    encryption: EncryptionConfig,
}

#[derive(Clone)]
pub struct S3Storage {
    bucket: Box<Bucket>,
    encryption: EncryptionConfig,
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

        Self {
            bucket,
            encryption: config.encryption,
        }
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

        let info =
            crate::encryption::info::event_info_from_filename(&event.metadata.get_filename());
        let data = self.encryption.event.encrypt(info, data.into())?;

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
            let data = response.bytes().to_owned();

            let info = crate::encryption::info::event_info_from_filename(filename);
            let data = self.encryption.event.decrypt(info, data)?;

            Ok(serde_json::from_slice(&data)?)
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

        let info =
            crate::encryption::info::segment_info_from_camera_and_filename(camera_name, filename);
        let data = self.encryption.segment.encrypt(info, data)?;

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
            let data = response.bytes().to_owned();

            let info = crate::encryption::info::segment_info_from_camera_and_filename(
                camera_name,
                filename,
            );
            let data = self.encryption.segment.decrypt(info, data)?;

            Ok(data)
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
        let id = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
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
                    .create_provider();

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
                    .create_provider();

                    crate::providers::test::$test(provider).await;
                }
            };
        }

        crate::providers::test::all_storage_tests!(test);
    }
}
