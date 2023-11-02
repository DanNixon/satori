use crate::{Provider, StorageError, StorageProvider, StorageResult};
use satori_common::Event;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use tracing::{info, warn};

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UnreferencedSegments {
    #[serde(flatten)]
    inner: HashMap<String, Vec<PathBuf>>,
}

impl UnreferencedSegments {
    pub fn save(&self, file: &Path) -> StorageResult<()> {
        let mut file = File::create(file)?;
        let report = toml::to_string_pretty(self)?;
        Ok(write!(file, "{}", report)?)
    }

    pub fn load(file: &Path) -> StorageResult<Self> {
        Ok(toml::from_str(&std::fs::read_to_string(file)?)?)
    }
}

pub async fn calculate_unreferenced_segments(
    storage: Provider,
) -> StorageResult<UnreferencedSegments> {
    info!("Getting camera list");
    let cameras = storage.list_cameras().await?;

    info!("Getting segment list(s)");
    let mut camera_segment_cache: HashMap<String, Vec<PathBuf>> = HashMap::new();
    // For each camera that has segments in the archive
    for camera in &cameras {
        info!("Getting segment list for camera \"{camera}\"");
        // Get a list of all segments stored for the camera
        camera_segment_cache.insert(camera.clone(), storage.list_segments(camera).await?);
    }

    info!("Getting event list");
    let event_filenames = storage.list_events().await?;

    info!(
        "Calculating referenced segments (from {} events)",
        event_filenames.len()
    );
    let mut referenced_segments = UniqueCameraSegmentCollection::default();
    for filename in event_filenames {
        info!("Processing event {}", filename.display());
        let event = storage.get_event(&filename).await?;
        referenced_segments.add_from_event(event);
    }

    let mut all_unreferenced_segments = UnreferencedSegments::default();

    // For each camera that has segments in the archive
    for camera in cameras {
        // Get the list of events for this camera from the cache
        let camera_segments = camera_segment_cache
            .remove(&camera)
            .expect("camera should be in segment cache");

        // Get referenced segments for camera
        info!("Calculating unreferenced segments for camera \"{camera}\"");
        let unreferenced_segments = match referenced_segments.get_segments_for_camera(&camera) {
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
            tx.send(s).await.expect("task channel should not be closed");
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
                    info!(
                        "(worker {worker_idx}) Deleting segment {}",
                        segment.display()
                    );

                    if let Err(err) = storage.delete_segment(&camera, &segment).await {
                        result = Err(StorageError::WorkflowPartialError);
                        warn!(
                            "Failed to delete segment {}, error: {err}",
                            segment.display()
                        );
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

#[derive(Debug, Default)]
struct UniqueCameraSegmentCollection {
    inner: HashMap<String, HashSet<PathBuf>>,
}

impl UniqueCameraSegmentCollection {
    fn add_from_event(&mut self, event: Event) {
        for camera in event.cameras {
            if !self.inner.contains_key(&camera.name) {
                self.inner.insert(camera.name.clone(), HashSet::new());
            }

            let segments = self.inner.get_mut(&camera.name).unwrap();
            for segment in camera.segment_list {
                segments.insert(segment);
            }
        }
    }

    fn get_segments_for_camera(&self, camera_name: &str) -> Option<&HashSet<PathBuf>> {
        self.inner.get(camera_name)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::providers::dummy::DummyConfig;
    use bytes::Bytes;
    use chrono::Utc;
    use satori_common::{CameraSegments, EventMetadata};
    use std::path::{Path, PathBuf};

    async fn build_test_storage() -> Provider {
        let provider = crate::StorageConfig::Dummy(DummyConfig::default()).create_provider();

        provider
            .put_segment("camera1", Path::new("1_1.ts"), Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera1", Path::new("1_2.ts"), Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera1", Path::new("1_3.ts"), Bytes::default())
            .await
            .unwrap();

        provider
            .put_segment("camera2", Path::new("2_1.ts"), Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera2", Path::new("2_2.ts"), Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera2", Path::new("2_3.ts"), Bytes::default())
            .await
            .unwrap();

        provider
            .put_segment("camera3", Path::new("3_1.ts"), Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera3", Path::new("3_2.ts"), Bytes::default())
            .await
            .unwrap();
        provider
            .put_segment("camera3", Path::new("3_3.ts"), Bytes::default())
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
                            PathBuf::from("1_1.ts"),
                            PathBuf::from("1_2.ts"),
                            PathBuf::from("1_3.ts"),
                        ],
                    },
                    CameraSegments {
                        name: "camera3".into(),
                        segment_list: vec![
                            PathBuf::from("3_1.ts"),
                            PathBuf::from("3_2.ts"),
                            PathBuf::from("3_3.ts"),
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
                        PathBuf::from("2_1.ts"),
                        PathBuf::from("2_2.ts"),
                        PathBuf::from("2_3.ts"),
                    ],
                }],
            })
            .await
            .unwrap();

        let unreferenced_segments = calculate_unreferenced_segments(provider.clone())
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
                Path::new("1_1.ts").to_owned(),
                Path::new("1_2.ts").to_owned(),
                Path::new("1_3.ts").to_owned(),
            ]
        );
        assert_eq!(
            provider.list_segments("camera2").await.unwrap(),
            vec![
                Path::new("2_1.ts").to_owned(),
                Path::new("2_2.ts").to_owned(),
                Path::new("2_3.ts").to_owned(),
            ]
        );
        assert_eq!(
            provider.list_segments("camera3").await.unwrap(),
            vec![
                Path::new("3_1.ts").to_owned(),
                Path::new("3_2.ts").to_owned(),
                Path::new("3_3.ts").to_owned(),
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
                        PathBuf::from("1_1.ts"),
                        PathBuf::from("1_2.ts"),
                        PathBuf::from("1_3.ts"),
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
                    segment_list: vec![PathBuf::from("2_2.ts"), PathBuf::from("2_3.ts")],
                }],
            })
            .await
            .unwrap();

        let unreferenced_segments = calculate_unreferenced_segments(provider.clone())
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
                Path::new("1_1.ts").to_owned(),
                Path::new("1_2.ts").to_owned(),
                Path::new("1_3.ts").to_owned(),
            ]
        );
        assert_eq!(
            provider.list_segments("camera2").await.unwrap(),
            vec![
                Path::new("2_2.ts").to_owned(),
                Path::new("2_3.ts").to_owned(),
            ]
        );
    }
}
