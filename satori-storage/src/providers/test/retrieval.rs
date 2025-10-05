use crate::{Provider, StorageProvider};
use bytes::Bytes;
use chrono::Utc;
use satori_common::{Event, EventMetadata};
use std::path::{Path, PathBuf};

pub(crate) async fn test_event_getters(provider: Provider) {
    let event1 = Event {
        metadata: EventMetadata {
            id: "test-1".into(),
            timestamp: Utc::now().into(),
        },
        start: Utc::now().into(),
        end: Utc::now().into(),
        reasons: Default::default(),
        cameras: Default::default(),
    };

    let event2 = Event {
        metadata: EventMetadata {
            id: "test-2".into(),
            timestamp: Utc::now().into(),
        },
        start: Utc::now().into(),
        end: Utc::now().into(),
        reasons: Default::default(),
        cameras: Default::default(),
    };

    provider.put_event(&event1).await.unwrap();
    provider.put_event(&event2).await.unwrap();

    assert_eq!(
        provider.list_events().await.unwrap(),
        vec![
            event1.metadata.get_filename(),
            event2.metadata.get_filename(),
        ]
    );

    assert_eq!(
        provider
            .get_event(&event1.metadata.get_filename())
            .await
            .unwrap(),
        event1
    );
}

pub(crate) async fn test_segment_getters(provider: Provider) {
    provider
        .put_segment("camera1", Path::new("1_1.ts"), Bytes::from("camera1_one"))
        .await
        .unwrap();
    provider
        .put_segment("camera1", Path::new("1_2.ts"), Bytes::from("camera1_two"))
        .await
        .unwrap();
    provider
        .put_segment("camera1", Path::new("1_3.ts"), Bytes::from("camera1_three"))
        .await
        .unwrap();

    provider
        .put_segment("camera2", Path::new("2_1.ts"), Bytes::from("camera2_onw"))
        .await
        .unwrap();
    provider
        .put_segment("camera2", Path::new("2_2.ts"), Bytes::from("camera2_two"))
        .await
        .unwrap();
    provider
        .put_segment("camera2", Path::new("2_3.ts"), Bytes::from("camera2_three"))
        .await
        .unwrap();

    provider
        .put_segment("camera3", Path::new("3_1.ts"), Bytes::from("camera3_one"))
        .await
        .unwrap();
    provider
        .put_segment("camera3", Path::new("3_2.ts"), Bytes::from("camera3_two"))
        .await
        .unwrap();
    provider
        .put_segment("camera3", Path::new("3_3.ts"), Bytes::from("camera3_three"))
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
        provider.list_segments("camera2").await.unwrap(),
        vec![
            Path::new("2_1.ts").to_owned(),
            Path::new("2_2.ts").to_owned(),
            Path::new("2_3.ts").to_owned(),
        ]
    );

    assert_eq!(
        provider
            .get_segment("camera2", &PathBuf::from("2_3.ts"))
            .await
            .unwrap(),
        Bytes::from("camera2_three"),
    );
}
