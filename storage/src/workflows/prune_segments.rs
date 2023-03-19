use crate::{Provider, StorageProvider, StorageResult};
use satori_common::Event;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};
use tracing::info;

pub async fn prune_unreferenced_segments(storage: Provider) -> StorageResult<()> {
    info!("Getting event list");
    let event_filenames = storage.list_events().await?;

    info!("Calculating referenced segments");
    let mut referenced_segments = CameraSegmentCollection::default();
    for filename in event_filenames {
        let event = storage.get_event(&filename).await?;
        referenced_segments.add_from_event(event);
    }

    // For each camera that has segments in the archive
    let cameras = storage.list_cameras().await?;
    for camera in cameras {
        // Get a list of all segments stored for the camera
        let camera_segments = storage.list_segments(&camera).await?;

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

        // Delete the unreferenced segments
        for s in unreferenced_segments {
            info!("Pruning segment: {}", s.display());
            storage.delete_segment(&camera, &s).await?;
        }
    }

    Ok(())
}

#[derive(Debug, Default)]
struct CameraSegmentCollection {
    inner: HashMap<String, HashSet<PathBuf>>,
}

impl CameraSegmentCollection {
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

        prune_unreferenced_segments(provider.clone()).await.unwrap();

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

        prune_unreferenced_segments(provider.clone()).await.unwrap();

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
