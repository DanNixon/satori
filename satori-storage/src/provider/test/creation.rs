use crate::Provider;
use bytes::Bytes;
use chrono::Utc;
use satori_common::{Event, EventMetadata};

pub(crate) async fn test_add_first_event(provider: Provider) {
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

    provider.put_event(&event1).await.unwrap();

    assert_eq!(
        provider.list_events().await.unwrap(),
        vec![event1.metadata.filename()]
    );

    assert_eq!(
        provider
            .get_event(&event1.metadata.filename())
            .await
            .unwrap(),
        event1
    );
}

pub(crate) async fn test_add_event(provider: Provider) {
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

    provider.put_event(&event1).await.unwrap();

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

    provider.put_event(&event2).await.unwrap();

    assert_eq!(
        provider.list_events().await.unwrap(),
        vec![event1.metadata.filename(), event2.metadata.filename(),]
    );

    assert_eq!(
        provider
            .get_event(&event2.metadata.filename())
            .await
            .unwrap(),
        event2
    );
}

pub(crate) async fn test_add_segment_new_camera(provider: Provider) {
    provider
        .put_segment("camera1", "1.ts", Bytes::default())
        .await
        .unwrap();

    assert_eq!(
        provider.list_segments("camera1").await.unwrap(),
        vec!["1.ts".to_owned(),]
    );
}

pub(crate) async fn test_add_segment_existing_camera(provider: Provider) {
    provider
        .put_segment("camera1", "1.ts", Bytes::default())
        .await
        .unwrap();

    provider
        .put_segment("camera1", "2.ts", Bytes::default())
        .await
        .unwrap();

    assert_eq!(
        provider.list_segments("camera1").await.unwrap(),
        vec!["1.ts".to_owned(), "2.ts".to_owned()]
    );
}
