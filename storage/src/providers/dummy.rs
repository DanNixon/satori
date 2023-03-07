use crate::{StorageError, StorageProvider, StorageResult};
use async_trait::async_trait;
use bytes::Bytes;
use satori_common::Event;
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

#[derive(Debug, Default, Deserialize)]
struct State {
    events: HashMap<PathBuf, Event>,
    segments: HashMap<String, HashMap<PathBuf, Bytes>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct DummyConfig {
    initial_state: State,
}

#[derive(Clone)]
pub struct DummyStorage {
    state: Arc<Mutex<State>>,
}

impl DummyStorage {
    pub fn new(config: DummyConfig) -> Self {
        Self {
            state: Arc::new(Mutex::new(config.initial_state)),
        }
    }
}

#[async_trait]
impl StorageProvider for DummyStorage {
    #[tracing::instrument(skip(self))]
    async fn put_event(&self, event: &Event) -> StorageResult<()> {
        self.state
            .lock()
            .unwrap()
            .events
            .insert(event.metadata.get_filename(), event.to_owned());
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list_events(&self) -> StorageResult<Vec<PathBuf>> {
        let mut events: Vec<PathBuf> = self
            .state
            .lock()
            .unwrap()
            .events
            .keys()
            .map(|k| k.to_owned())
            .collect();
        events.sort();
        Ok(events)
    }

    #[tracing::instrument(skip(self))]
    async fn get_event(&self, filename: &Path) -> StorageResult<Event> {
        self.state
            .lock()
            .unwrap()
            .events
            .get(filename)
            .cloned()
            .ok_or(StorageError::NotFound)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_event(&self, event: &Event) -> StorageResult<()> {
        self.delete_event_filename(&event.metadata.get_filename())
            .await
    }

    #[tracing::instrument(skip(self))]
    async fn delete_event_filename(&self, filename: &Path) -> StorageResult<()> {
        self.state.lock().unwrap().events.remove(filename);
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list_cameras(&self) -> StorageResult<Vec<String>> {
        let mut cameras: Vec<String> = self
            .state
            .lock()
            .unwrap()
            .segments
            .keys()
            .map(|k| k.to_owned())
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
        let mut state = self.state.lock().unwrap();

        if !state.segments.contains_key(camera_name) {
            state
                .segments
                .insert(camera_name.into(), HashMap::default());
        }

        state
            .segments
            .get_mut(camera_name)
            .unwrap()
            .insert(filename.into(), data);

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list_segments(&self, camera_name: &str) -> StorageResult<Vec<PathBuf>> {
        let mut segments: Vec<PathBuf> = self
            .state
            .lock()
            .unwrap()
            .segments
            .get(camera_name)
            .ok_or(StorageError::NotFound)?
            .iter()
            .map(|i| i.0.to_owned())
            .collect();
        segments.sort();
        Ok(segments)
    }

    #[tracing::instrument(skip(self))]
    async fn get_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<Bytes> {
        Ok(self
            .state
            .lock()
            .unwrap()
            .segments
            .get(camera_name)
            .ok_or(StorageError::NotFound)?
            .get(filename)
            .ok_or(StorageError::NotFound)?
            .to_owned())
    }

    #[tracing::instrument(skip(self))]
    async fn delete_segment(&self, camera_name: &str, filename: &Path) -> StorageResult<()> {
        let mut state = self.state.lock().unwrap();
        let camera_segments = state
            .segments
            .get_mut(camera_name)
            .ok_or(StorageError::NotFound)?;
        camera_segments.retain(|k, _| k != filename);
        if camera_segments.is_empty() {
            state.segments.remove(camera_name);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::super::test as storage_tests;
    use super::*;
    use crate::Provider;

    async fn run_test<Fut>(test_func: impl FnOnce(Provider) -> Fut)
    where
        Fut: std::future::Future<Output = ()>,
    {
        let provider =
            super::super::super::StorageConfig::Dummy(DummyConfig::default()).create_provider();
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
