use crate::{Provider, StorageError, StorageResult};
use satori_common::Event;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
};
use tracing::{info, warn};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UnreferencedSegments {
    #[serde(flatten)]
    inner: HashMap<String, Vec<String>>,
}

impl UnreferencedSegments {
    pub fn save(&self, file: &Path) -> StorageResult<()> {
        let mut file = File::create(file)?;
        let report = toml::to_string_pretty(self)?;
        Ok(write!(file, "{report}")?)
    }

    pub fn load(file: &Path) -> StorageResult<Self> {
        Ok(toml::from_str(&std::fs::read_to_string(file)?)?)
    }
}

/// Retrieves a list of segments that are referred to by any event in a given storage provider.
///
/// 1. Retrieves the list of all events
/// 2. For each event
///     1. Retrieve the full event data
///     2. For each camera in the event
///         1. Extend the set of referenced segments for this camera
async fn get_referenced_segments(
    storage: Provider,
    num_workers: usize,
) -> StorageResult<UniqueCameraSegmentCollection> {
    info!("Getting event list");
    let event_filenames = storage.list_events().await?;

    info!(
        "Calculating referenced segments (from {} events)",
        event_filenames.len()
    );
    let referenced_segments = UniqueCameraSegmentCollection::default();

    // Channel that forms the job queue for workers
    let (tx, rx) = async_channel::unbounded();

    // Fill the channel with the event filenames then immediately close it
    // Workers will terminate when the channel is empty and closed
    for filename in event_filenames {
        tx.send(filename)
            .await
            .expect("task channel should be open");
    }
    tx.close();

    // Start as many workers as were requested
    let mut workers = Vec::new();
    for worker_idx in 0..num_workers {
        let storage = storage.clone();
        let rx = rx.clone();
        let referenced_segments = referenced_segments.clone();

        workers.push(tokio::spawn(async move {
            while let Ok(filename) = rx.recv().await {
                info!("(worker {worker_idx}) Processing event {filename}");

                // Attempt to get event data
                match storage.get_event(&filename).await {
                    Ok(event) => {
                        referenced_segments.add_from_event(event);
                    }
                    Err(err) => {
                        warn!("Failed to retrieve event {filename}, error: {err}");
                        return Err(StorageError::WorkflowPartialError);
                    }
                };
            }

            Ok(())
        }));
    }

    // Wait for all workers to terminate, collecting results and returning an error if any one job
    // failed
    if futures::future::join_all(workers)
        .await
        .iter()
        .any(|r| match r {
            Err(_) => true,
            Ok(Err(_)) => true,
            Ok(_) => false,
        })
    {
        Err(StorageError::WorkflowPartialError)
    } else {
        Ok(referenced_segments)
    }
}

pub async fn calculate_unreferenced_segments(
    storage: Provider,
    num_workers: usize,
) -> StorageResult<UnreferencedSegments> {
    info!("Getting camera list");
    let cameras = storage.list_cameras().await?;

    info!("Getting segment list(s)");
    let mut camera_segment_cache: HashMap<String, Vec<String>> = HashMap::new();
    // For each camera that has segments in the archive
    for camera in &cameras {
        info!("Getting segment list for camera \"{camera}\"");
        // Get a list of all segments stored for the camera
        camera_segment_cache.insert(camera.clone(), storage.list_segments(camera).await?);
    }

    let referenced_segments = get_referenced_segments(storage, num_workers).await?;

    let mut all_unreferenced_segments = UnreferencedSegments::default();

    // For each camera that has segments in the archive
    for camera in cameras {
        // Get the list of events for this camera from the cache
        let camera_segments = camera_segment_cache
            .remove(&camera)
            .expect("camera should be in segment cache");

        // Get referenced segments for camera
        info!("Calculating unreferenced segments for camera \"{camera}\"");
        let unreferenced_segments = match referenced_segments.inner.lock().unwrap().get(&camera) {
            // Remove referenced segments from the list of stored segments
            Some(referenced_segments) => camera_segments
                .into_iter()
                .filter(|s| !referenced_segments.contains(s))
                .collect(),
            // Use entire stored segments list if no referenced segments for camera exist
            None => camera_segments,
        };

        // Record the unreferenced segments
        all_unreferenced_segments
            .inner
            .insert(camera, unreferenced_segments);
    }

    Ok(all_unreferenced_segments)
}

pub async fn delete_unreferenced_segments(
    storage: Provider,
    unreferenced_segments: UnreferencedSegments,
    num_workers: usize,
) -> StorageResult<()> {
    let mut results = Vec::new();

    for (camera, segments) in unreferenced_segments.inner {
        info!("Pruning segments for \"{camera}\"");

        let (tx, rx) = async_channel::unbounded();

        for s in segments {
            tx.send(s).await.expect("task channel should be open");
        }
        tx.close();

        let mut workers = Vec::new();
        for worker_idx in 0..num_workers {
            let storage = storage.clone();
            let camera = camera.clone();
            let rx = rx.clone();

            workers.push(tokio::spawn(async move {
                let mut result = Ok(());

                while let Ok(segment) = rx.recv().await {
                    info!("(worker {worker_idx}) Deleting segment {segment}");

                    if let Err(err) = storage.delete_segment(&camera, &segment).await {
                        result = Err(StorageError::WorkflowPartialError);
                        warn!("Failed to delete segment {segment}, error: {err}");
                    }
                }

                result
            }));
        }

        results.push(
            if futures::future::join_all(workers)
                .await
                .iter()
                .any(|r| match r {
                    Err(_) => true,
                    Ok(Err(_)) => true,
                    Ok(_) => false,
                })
            {
                Err(StorageError::WorkflowPartialError)
            } else {
                Ok(())
            },
        );
    }

    if results.iter().any(|r| r.is_err()) {
        Err(StorageError::WorkflowPartialError)
    } else {
        Ok(())
    }
}

#[derive(Debug, Default, Clone)]
struct UniqueCameraSegmentCollection {
    inner: Arc<Mutex<HashMap<String, HashSet<String>>>>,
}

impl UniqueCameraSegmentCollection {
    fn add_from_event(&self, event: Event) {
        let mut inner = self.inner.lock().unwrap();

        for camera in event.cameras {
            if !inner.contains_key(&camera.name) {
                inner.insert(camera.name.clone(), HashSet::new());
            }

            let segments = inner.get_mut(&camera.name).unwrap();
            for segment in camera.segment_list {
                segments.insert(segment);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::EncryptionConfig;
    use bytes::Bytes;
    use chrono::Utc;
    use satori_common::{CameraSegments, EventMetadata};
    use url::Url;

    async fn build_test_storage() -> Provider {
        let provider = Provider::new(
            Url::parse("memory:///").unwrap(),
            EncryptionConfig::default(),
        )
        .unwrap();

        provider
            .put_segment("camera1", "1_1.ts", Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera1", "1_2.ts", Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera1", "1_3.ts", Bytes::default())
            .await
            .unwrap();

        provider
            .put_segment("camera2", "2_1.ts", Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera2", "2_2.ts", Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera2", "2_3.ts", Bytes::default())
            .await
            .unwrap();

        provider
            .put_segment("camera3", "3_1.ts", Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera3", "3_2.ts", Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera3", "3_3.ts", Bytes::default())
            .await
            .unwrap();

        provider
    }

    #[tokio::test]
    async fn test_prune_segments_noop() {
        let provider = build_test_storage().await;

        provider
            .put_event(&Event {
                metadata: EventMetadata {
                    id: "test-1".into(),
                    timestamp: Utc::now().into(),
                },
                start: Utc::now().into(),
                end: Utc::now().into(),
                reasons: Default::default(),
                cameras: vec![
                    CameraSegments {
                        name: "camera1".into(),
                        segment_list: vec![
                            "1_1.ts".to_owned(),
                            "1_2.ts".to_owned(),
                            "1_3.ts".to_owned(),
                        ],
                    },
                    CameraSegments {
                        name: "camera3".into(),
                        segment_list: vec![
                            "3_1.ts".to_owned(),
                            "3_2.ts".to_owned(),
                            "3_3.ts".to_owned(),
                        ],
                    },
                ],
            })
            .await
            .unwrap();

        provider
            .put_event(&Event {
                metadata: EventMetadata {
                    id: "test-2".into(),
                    timestamp: Utc::now().into(),
                },
                start: Utc::now().into(),
                end: Utc::now().into(),
                reasons: Default::default(),
                cameras: vec![CameraSegments {
                    name: "camera2".into(),
                    segment_list: vec![
                        "2_1.ts".to_owned(),
                        "2_2.ts".to_owned(),
                        "2_3.ts".to_owned(),
                    ],
                }],
            })
            .await
            .unwrap();

        let unreferenced_segments = calculate_unreferenced_segments(provider.clone(), 2)
            .await
            .unwrap();

        delete_unreferenced_segments(provider.clone(), unreferenced_segments, 2)
            .await
            .unwrap();

        assert_eq!(
            provider.list_cameras().await.unwrap(),
            vec![
                "camera1".to_string(),
                "camera2".to_string(),
                "camera3".to_string(),
            ]
        );

        assert_eq!(
            provider.list_segments("camera1").await.unwrap(),
            vec![
                "1_1.ts".to_owned(),
                "1_2.ts".to_owned(),
                "1_3.ts".to_owned(),
            ]
        );
        assert_eq!(
            provider.list_segments("camera2").await.unwrap(),
            vec![
                "2_1.ts".to_owned(),
                "2_2.ts".to_owned(),
                "2_3.ts".to_owned(),
            ]
        );
        assert_eq!(
            provider.list_segments("camera3").await.unwrap(),
            vec![
                "3_1.ts".to_owned(),
                "3_2.ts".to_owned(),
                "3_3.ts".to_owned(),
            ]
        );
    }

    #[tokio::test]
    async fn test_prune_segments() {
        let provider = build_test_storage().await;

        provider
            .put_event(&Event {
                metadata: EventMetadata {
                    id: "test-1".into(),
                    timestamp: Utc::now().into(),
                },
                start: Utc::now().into(),
                end: Utc::now().into(),
                reasons: Default::default(),
                cameras: vec![CameraSegments {
                    name: "camera1".into(),
                    segment_list: vec![
                        "1_1.ts".to_owned(),
                        "1_2.ts".to_owned(),
                        "1_3.ts".to_owned(),
                    ],
                }],
            })
            .await
            .unwrap();

        provider
            .put_event(&Event {
                metadata: EventMetadata {
                    id: "test-2".into(),
                    timestamp: Utc::now().into(),
                },
                start: Utc::now().into(),
                end: Utc::now().into(),
                reasons: Default::default(),
                cameras: vec![CameraSegments {
                    name: "camera2".into(),
                    segment_list: vec!["2_2.ts".to_owned(), "2_3.ts".to_owned()],
                }],
            })
            .await
            .unwrap();

        let unreferenced_segments = calculate_unreferenced_segments(provider.clone(), 2)
            .await
            .unwrap();

        delete_unreferenced_segments(provider.clone(), unreferenced_segments, 2)
            .await
            .unwrap();

        assert_eq!(
            provider.list_cameras().await.unwrap(),
            vec!["camera1".to_string(), "camera2".to_string(),]
        );

        assert_eq!(
            provider.list_segments("camera1").await.unwrap(),
            vec![
                "1_1.ts".to_owned(),
                "1_2.ts".to_owned(),
                "1_3.ts".to_owned(),
            ]
        );
        assert_eq!(
            provider.list_segments("camera2").await.unwrap(),
            vec!["2_2.ts".to_owned(), "2_3.ts".to_owned()]
        );
    }
}
