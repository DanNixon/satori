use crate::{Provider, StorageError, StorageProvider, StorageResult};
use bytes::{BufMut, Bytes};
use satori_common::{CameraSegments, Event};
use std::path::{Path, PathBuf};
use tracing::info;

pub fn generate_video_filename(
    event: &Event,
    camera_name: Option<String>,
) -> StorageResult<PathBuf> {
    let timestamp = event.metadata.timestamp.to_rfc3339();
    let camera = get_camera_from_event_by_name(event, camera_name)?;
    Ok(PathBuf::from(format!("{timestamp}_{0}.mp4", camera.name)))
}

pub async fn export_event_video(
    storage: Provider,
    event_filename: &Path,
    camera_name: Option<String>,
) -> StorageResult<(Event, Bytes)> {
    info!("Getting event: {}", event_filename.display());
    let event = storage.get_event(event_filename).await?;
    let camera = get_camera_from_event_by_name(&event, camera_name)?;
    let video_data = get_file_from_segments(storage, camera).await?;
    Ok((event, video_data))
}

fn get_camera_from_event_by_name(
    event: &Event,
    camera_name: Option<String>,
) -> StorageResult<&CameraSegments> {
    Ok(match camera_name {
        Some(camera_name) => event
            .cameras
            .iter()
            .find(|c| c.name == camera_name)
            .ok_or(StorageError::NoSuchCamera(camera_name))?,
        None => {
            if event.cameras.len() == 1 {
                &event.cameras[0]
            } else {
                return Err(StorageError::CameraMustBeSpecified);
            }
        }
    })
}

async fn get_file_from_segments(
    storage: Provider,
    camera: &CameraSegments,
) -> StorageResult<Bytes> {
    let mut file_content: Vec<u8> = Vec::new();

    for segment_filename in &camera.segment_list {
        info!("Getting segment: {}", segment_filename.display());
        file_content.put(storage.get_segment(&camera.name, segment_filename).await?);
    }

    Ok(file_content.into())
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::providers::dummy::DummyConfig;
    use bytes::Bytes;
    use chrono::Utc;
    use satori_common::{Event, EventMetadata};
    use std::path::PathBuf;

    #[test]
    fn test_generate_video_filename_default_camera() {
        let event = Event {
            metadata: EventMetadata {
                id: "test".into(),
                timestamp: chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                    .unwrap()
                    .and_hms_opt(18, 8, 0)
                    .unwrap()
                    .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                    .unwrap(),
            },
            start: Utc::now().into(),
            end: Utc::now().into(),
            reasons: Default::default(),
            cameras: vec![CameraSegments {
                name: "camera1".into(),
                segment_list: vec![PathBuf::from("1_2.ts"), PathBuf::from("1_3.ts")],
            }],
        };

        assert_eq!(
            generate_video_filename(&event, None).unwrap(),
            PathBuf::from("2022-12-30T18:08:00+00:00_camera1.mp4")
        );
    }

    #[test]
    fn test_generate_video_filename_specified_camera() {
        let event = Event {
            metadata: EventMetadata {
                id: "test".into(),
                timestamp: chrono::NaiveDate::from_ymd_opt(2022, 12, 30)
                    .unwrap()
                    .and_hms_opt(18, 8, 0)
                    .unwrap()
                    .and_local_timezone(chrono::FixedOffset::east_opt(0).unwrap())
                    .unwrap(),
            },
            start: Utc::now().into(),
            end: Utc::now().into(),
            reasons: Default::default(),
            cameras: vec![
                CameraSegments {
                    name: "camera1".into(),
                    segment_list: vec![PathBuf::from("1_2.ts"), PathBuf::from("1_3.ts")],
                },
                CameraSegments {
                    name: "camera2".into(),
                    segment_list: vec![PathBuf::from("2_2.ts"), PathBuf::from("2_3.ts")],
                },
            ],
        };

        assert_eq!(
            generate_video_filename(&event, Some("camera2".into())).unwrap(),
            PathBuf::from("2022-12-30T18:08:00+00:00_camera2.mp4")
        );
    }

    #[tokio::test]
    async fn test_export_event_video() {
        let provider = crate::StorageConfig::Dummy(DummyConfig::default()).create_provider();

        provider
            .put_segment("camera1", Path::new("1_1.ts"), Bytes::from("one"))
            .await
            .unwrap();
        provider
            .put_segment("camera1", Path::new("1_2.ts"), Bytes::from("two"))
            .await
            .unwrap();
        provider
            .put_segment("camera1", Path::new("1_3.ts"), Bytes::from("three"))
            .await
            .unwrap();

        let event = Event {
            metadata: EventMetadata {
                id: "test".into(),
                timestamp: Utc::now().into(),
            },
            start: Utc::now().into(),
            end: Utc::now().into(),
            reasons: Default::default(),
            cameras: vec![CameraSegments {
                name: "camera1".into(),
                segment_list: vec![PathBuf::from("1_2.ts"), PathBuf::from("1_3.ts")],
            }],
        };

        provider.put_event(&event).await.unwrap();

        let (returned_event, video_bytes) = export_event_video(
            provider,
            &event.metadata.get_filename(),
            Some("camera1".into()),
        )
        .await
        .unwrap();

        assert_eq!(returned_event, event);
        assert_eq!(video_bytes, Bytes::from("twothree"));
    }
}
