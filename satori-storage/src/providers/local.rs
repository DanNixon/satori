use crate::{EncryptionConfig, StorageProvider, StorageResult, encryption::KeyOperations};
use async_trait::async_trait;
use bytes::Bytes;
use satori_common::Event;
use serde::Deserialize;
use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tracing::warn;

#[derive(Debug, Deserialize)]
pub struct LocalConfig {
    path: PathBuf,
    #[serde(default)]
    encryption: EncryptionConfig,
}

#[derive(Clone)]
pub struct LocalStorage {
    event_directory: PathBuf,
    segment_directory: PathBuf,
    encryption: EncryptionConfig,
}

impl LocalStorage {
    pub fn new(config: LocalConfig) -> Self {
        let event_directory = config.path.join("events");
        let segment_directory = config.path.join("segments");

        let storage = Self {
            event_directory,
            segment_directory,
            encryption: config.encryption,
        };

        storage.make_directories();

        storage
    }

    fn make_directories(&self) {
        std::fs::create_dir_all(&self.event_directory).unwrap();
        std::fs::create_dir_all(&self.segment_directory).unwrap();
    }

    fn get_event_filename(&self, event: &Event) -> PathBuf {
        self.event_directory.join(event.metadata.get_filename())
    }

    fn get_segment_directory(&self, camera_name: &str) -> PathBuf {
        self.segment_directory.join(camera_name)
    }

    fn get_segment_filename(&self, camera_name: &str, filename: &Path) -> PathBuf {
        self.get_segment_directory(camera_name).join(filename)
    }
}

#[async_trait]
impl StorageProvider for LocalStorage {
    #[tracing::instrument(skip(self))]
    async fn put_event(&self, event: &Event) -> StorageResult<()> {
        let info =
            crate::encryption::info::event_info_from_filename(&event.metadata.get_filename());

        let filename = self.get_event_filename(event);
        let mut file = File::create(filename)?;

        let data = serde_json::to_vec_pretty(&event)?;

        let data = self.encryption.event.encrypt(info, data.into())?;

        file.write_all(&data)?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list_events(&self) -> StorageResult<Vec<PathBuf>> {
        list_dir(&self.event_directory, "json")
    }

    #[tracing::instrument(skip(self))]
    async fn get_event(&self, filename: &Path) -> StorageResult<Event> {
        let info = crate::encryption::info::event_info_from_filename(filename);

        let filename = self.event_directory.join(filename);
        let mut file = File::open(filename)?;

        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        let data = self.encryption.event.decrypt(info, data.into())?;

        Ok(serde_json::from_slice(&data)?)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_event(&self, event: &Event) -> StorageResult<()> {
        let filename = self.get_event_filename(event);
        self.delete_event_filename(&filename).await
    }

    #[tracing::instrument(skip(self))]
    async fn delete_event_filename(&self, filename: &Path) -> StorageResult<()> {
        let filename = self.event_directory.join(filename);
        std::fs::remove_file(filename)?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list_cameras(&self) -> StorageResult<Vec<String>> {
        list_dir_dirs(&self.segment_directory)
    }

    #[tracing::instrument(skip(self, data))]
    async fn put_segment(
        &self,
        camera_name: &str,
        filename: &Path,
        data: Bytes,
    ) -> StorageResult<()> {
        let info =
            crate::encryption::info::segment_info_from_camera_and_filename(camera_name, filename);

        let dir = self.get_segment_directory(camera_name);
        std::fs::create_dir_all(&dir)?;

        let filename = dir.join(filename);
        let mut file = File::create(filename)?;

        let data = self.encryption.segment.encrypt(info, data)?;
        file.write_all(&data)?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list_segments(&self, camera_name: &str) -> StorageResult<Vec<PathBuf>> {
        let dir = self.get_segment_directory(camera_name);
        list_dir(&dir, "ts")
    }

    #[tracing::instrument(skip(self))]
    async fn get_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<Bytes> {
        let info =
            crate::encryption::info::segment_info_from_camera_and_filename(camera_name, filename);

        let filename = self.get_segment_filename(camera_name, filename);

        let mut file = File::open(filename)?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)?;

        let data = self.encryption.segment.decrypt(info, data.into())?;

        Ok(data)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<()> {
        let filename = self.get_segment_filename(camera_name, filename);
        std::fs::remove_file(filename)?;

        let camera_directory = self.get_segment_directory(camera_name);
        // Check if the directory is empty
        if camera_directory
            .read_dir()
            .map(|mut i| i.next().is_none())
            .unwrap_or(false)
            && let Err(err) = std::fs::remove_dir(&camera_directory)
        {
            warn!(
                "Failed to remove directory ({}) for camera that no longer has any video segments. {err}",
                camera_directory.display()
            );
        }

        Ok(())
    }
}

#[tracing::instrument]
fn list_dir(dir: &Path, ext: &str) -> StorageResult<Vec<PathBuf>> {
    let mut contents: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|p| match p.as_ref() {
            Ok(p) => {
                let md = p.path();
                if md.is_file() && md.extension() == Some(std::ffi::OsStr::new(ext)) {
                    Some(md.file_name().unwrap().into())
                } else {
                    None
                }
            }
            Err(_) => None,
        })
        .collect();
    contents.sort();
    Ok(contents)
}

#[tracing::instrument]
fn list_dir_dirs(dir: &Path) -> StorageResult<Vec<String>> {
    let mut contents: Vec<String> = std::fs::read_dir(dir)?
        .filter_map(|p| match p.as_ref() {
            Ok(p) => {
                let md = p.path();
                if md.is_dir() {
                    Some(
                        md.components()
                            .next_back()
                            .unwrap()
                            .as_os_str()
                            .to_str()
                            .unwrap()
                            .into(),
                    )
                } else {
                    None
                }
            }
            Err(_) => None,
        })
        .collect();
    contents.sort();
    Ok(contents)
}

#[cfg(test)]
mod test {
    use super::*;

    mod no_encryption {
        use super::*;

        macro_rules! test {
            ( $test:ident ) => {
                #[tokio::test]
                async fn $test() {
                    let temp_dir = tempfile::Builder::new()
                        .prefix("satori_local_storage_test")
                        .tempdir()
                        .unwrap();

                    let provider = crate::StorageConfig::Local(LocalConfig {
                        path: temp_dir.path().to_owned(),
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
                    let temp_dir = tempfile::Builder::new()
                        .prefix("satori_local_storage_test")
                        .tempdir()
                        .unwrap();

                    let provider = crate::StorageConfig::Local(LocalConfig {
                        path: temp_dir.path().to_owned(),
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
