use crate::{Provider, StorageProvider};
use bytes::Bytes;
use chrono::Utc;
use satori_common::{Event, EventMetadata};
use std::path::{Path, PathBuf};

pub(crate) async fn test_delete_event(provider: Provider) {
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

    provider.delete_event(&event1).await.unwrap();

    let mut events = provider.list_events().await.unwrap();
    events.sort();
    assert_eq!(events, vec![event2.metadata.get_filename(),]);
}

pub(crate) async fn test_delete_event_filename(provider: Provider) {
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

    provider
        .delete_event_filename(&event1.metadata.get_filename())
        .await
        .unwrap();

    let mut events = provider.list_events().await.unwrap();
    events.sort();
    assert_eq!(events, vec![event2.metadata.get_filename(),]);
}

pub(crate) async fn test_delete_segment(provider: Provider) {
    provider
        .put_segment("camera1", Path::new("1.ts"), Bytes::default())
        .await
        .unwrap();
    provider
        .put_segment("camera1", Path::new("2.ts"), Bytes::default())
        .await
        .unwrap();
    provider
        .put_segment("camera1", Path::new("3.ts"), Bytes::default())
        .await
        .unwrap();

    assert_eq!(
        provider.list_segments("camera1").await.unwrap(),
        vec![
            Path::new("1.ts").to_owned(),
            Path::new("2.ts").to_owned(),
            Path::new("3.ts").to_owned(),
        ]
    );

    provider
        .delete_segment("camera1", Path::new("2.ts"))
        .await
        .unwrap();

    assert_eq!(
        provider.list_segments("camera1").await.unwrap(),
        vec![Path::new("1.ts").to_owned(), Path::new("3.ts").to_owned(),]
    );
}

pub(crate) async fn test_delete_last_segment_deletes_camera(provider: Provider) {
    provider
        .put_segment("camera1", Path::new("1.ts"), Bytes::default())
        .await
        .unwrap();

    assert_eq!(provider.list_cameras().await.unwrap(), vec!["camera1"]);

    assert_eq!(
        provider.list_segments("camera1").await.unwrap(),
        vec![PathBuf::from("1.ts")]
    );

    provider
        .delete_segment("camera1", Path::new("1.ts"))
        .await
        .unwrap();

    assert!(provider.list_cameras().await.unwrap().is_empty());
}
