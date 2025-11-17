use crate::{archive::tasks::ArchiveTask, hls_client::HlsClient, segments::Playlist};
use miette::{Context, IntoDiagnostic};
use object_store::{ObjectStore, path::Path};
use satori_common::{CameraSegments, Event, EventReason, Trigger};
use std::{sync::Arc, time::Duration};
use tracing::{error, info, warn};
use url::Url;

pub(crate) struct EventSet {
    store: Arc<dyn ObjectStore>,
    event_ttl: Duration,

    events: Vec<Event>,
}

impl EventSet {
    #[tracing::instrument]
    pub(crate) async fn new(store: Arc<dyn ObjectStore>, event_ttl: Duration) -> Self {
        let events = match load_events_from_store(store.clone()).await {
            Ok(q) => q,
            Err(e) => {
                warn!("Failed to load active events from store: {e}");
                Vec::new()
            }
        };

        Self {
            store,
            event_ttl,
            events,
        }
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn save(&self) {
        if let Err(e) = save_events_to_store(self.store.clone(), &self.events).await {
            warn!("Failed to save active events to store: {e}");
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn trigger(&mut self, trigger: &Trigger) {
        crate::o11y::inc_triggers_metric(trigger.metadata.id.clone());

        match self
            .events
            .iter_mut()
            .find(|e| e.metadata.id == trigger.metadata.id)
        {
            Some(e) => {
                // If there is an event with the same ID then update it
                info!("Updating existing event matching trigger");
                update_event(e, trigger);
            }
            None => {
                // Otherwise add a new event
                info!("Adding new event for trigger");
                self.events.push(trigger.clone().into());
            }
        }

        self.save().await;
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn process(
        &mut self,
        camera_client: &HlsClient,
        storage_api_urls: &[Url],
        archive_task_tx: &tokio::sync::mpsc::UnboundedSender<ArchiveTask>,
    ) -> miette::Result<()> {
        for event in &mut self.events {
            info!("Processing event: {:?}", event.metadata);

            for camera in &mut event.cameras {
                info!("Processing camera: {}", camera.name);

                // Retrieve playlist
                let playlist: Playlist = match camera_client.get_playlist(&camera.name).await {
                    Ok(response) => response.into(),
                    Err(err) => {
                        error!(
                            "Failed to get segments for {}, reason: {}",
                            camera.name, err
                        );
                        continue;
                    }
                };

                // Filter segments that are in event time frame
                let segments = playlist.between(event.start, event.end);

                // Compute set of new segments (that are not already recorded in event)
                let mut new_segments: Vec<String> = segments
                    .into_iter()
                    .filter_map(|s| {
                        if camera.segment_list.contains(&s.filename) {
                            None
                        } else {
                            Some(s.filename.clone())
                        }
                    })
                    .collect();
                info!(
                    "Found {} new segment(s) for {}",
                    new_segments.len(),
                    camera.name
                );

                if !new_segments.is_empty() {
                    for segment in &new_segments {
                        let segment_url = camera_client
                            .get_camera_url(&camera.name)
                            .unwrap()
                            .join(segment)
                            .unwrap();

                        // Create and send the segment archive tasks for this camera
                        for task in ArchiveTask::new_segment(
                            storage_api_urls,
                            camera.name.clone(),
                            segment_url,
                        ) {
                            archive_task_tx
                                .send(task)
                                .into_diagnostic()
                                .wrap_err("Failed to transmit archive task on channel")?;
                        }
                    }
                }

                // Update segment list in event
                camera.segment_list.append(&mut new_segments);
            }

            // Create and send the event archive task
            for task in ArchiveTask::new_event(storage_api_urls, event.clone()) {
                archive_task_tx
                    .send(task)
                    .into_diagnostic()
                    .wrap_err("Failed to transmit archive task on channel")?;
            }
        }

        // Now remove any events that have outlived the TTL
        self.prune_expired_events();

        metrics::gauge!(crate::o11y::ACTIVE_EVENTS).set(self.events.len() as f64);

        self.save().await;

        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn prune_expired_events(&mut self) {
        info!("Pruning expired events");

        self.events = self
            .events
            .iter()
            .filter_map(|event| {
                if event.should_expire(self.event_ttl) {
                    info!("Removing event: {:?}", event.metadata);
                    crate::o11y::inc_expired_events_metric(event.metadata.id.clone());
                    None
                } else {
                    Some(event.clone())
                }
            })
            .collect();

        info!("{} event(s) remain", self.events.len());
    }
}

const STORE_PATH: &str = "active_events.json";

async fn load_events_from_store<T: ObjectStore>(store: T) -> miette::Result<Vec<Event>> {
    let state_file = store.get(&Path::from(STORE_PATH)).await.into_diagnostic()?;
    let state_data = state_file.bytes().await.into_diagnostic()?;
    let queue: Vec<Event> = serde_json::from_slice(&state_data).into_diagnostic()?;
    info!("Loaded queue with {} entries from store", queue.len());
    Ok(queue)
}

async fn save_events_to_store<T: ObjectStore>(store: T, queue: &[Event]) -> miette::Result<()> {
    let bytes = serde_json::to_vec(queue).into_diagnostic()?;
    let _ = store
        .put(&Path::from(STORE_PATH), bytes.into())
        .await
        .into_diagnostic()?;
    info!("Saved queue to store");
    Ok(())
}

fn update_event(event: &mut Event, other: &Trigger) {
    if event.metadata.id != other.metadata.id {
        panic!("Event IDs should match");
    }

    // Update reason list.
    event.reasons.push(EventReason {
        timestamp: other.metadata.timestamp,
        reason: other.reason.clone(),
    });

    // Update start time.
    // Set new start time if it is earlier than the event's current start time.
    let other_start = other.start_time();
    if other_start < event.start {
        event.start = other_start;
    }

    // Update end time.
    // Set new end time if it is later than the event's current end time.
    let other_end = other.end_time();
    if other_end > event.end {
        event.end = other_end;
    }

    // Add any cameras not already in the event.
    let currnet_cams: Vec<String> = event.cameras.iter().map(|i| i.name.clone()).collect();
    for camera in &other.cameras {
        if !currnet_cams.contains(camera) {
            event.cameras.push(CameraSegments {
                name: camera.clone(),
                segment_list: Vec::new(),
            });
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::Utc;
    use object_store::memory::InMemory;
    use satori_common::EventMetadata;

    #[tokio::test]
    async fn test_load_bad_file_gives_empty_event_set() {
        let store = Arc::new(InMemory::new());
        let es = EventSet::new(store, Duration::default()).await;
        assert!(es.events.is_empty());
    }

    #[tokio::test]
    async fn test_trigger_1() {
        let store = Arc::new(InMemory::new());
        let mut es = EventSet::new(store, Duration::default()).await;

        // No events should exist
        assert!(es.events.is_empty());

        // Trigger with ID=trigger1
        es.trigger(&Trigger {
            metadata: EventMetadata {
                id: "trigger1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "".into(),
            cameras: Vec::default(),
            pre: Duration::from_secs(1),
            post: Duration::from_secs(2),
        })
        .await;
        es.prune_expired_events();

        // Event with ID=trigger1 should exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");

        std::thread::sleep(Duration::from_secs(1));
        es.prune_expired_events();

        // Event with ID=trigger1 should still exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");
        let previous_end_time = es.events[0].end;

        // Trigger with ID=trigger1
        es.trigger(&Trigger {
            metadata: EventMetadata {
                id: "trigger1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "".into(),
            cameras: Vec::default(),
            pre: Duration::from_secs(1),
            post: Duration::from_secs(2),
        })
        .await;
        es.prune_expired_events();

        // Event with ID=trigger1 should still exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");
        // And it should now have a later end time
        assert!(previous_end_time < es.events[0].end);

        std::thread::sleep(Duration::from_secs(1));
        es.prune_expired_events();

        // Event with ID=trigger1 should still exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");

        std::thread::sleep(Duration::from_secs(2));
        es.prune_expired_events();

        // No events should exist
        assert!(es.events.is_empty());
    }

    #[tokio::test]
    async fn test_trigger_2() {
        let store = Arc::new(InMemory::new());
        let mut es = EventSet::new(store, Duration::default()).await;

        // Trigger with ID=trigger1
        es.trigger(&Trigger {
            metadata: EventMetadata {
                id: "trigger1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "".into(),
            cameras: Vec::default(),
            pre: Duration::from_secs(1),
            post: Duration::from_secs(2),
        })
        .await;
        es.prune_expired_events();

        // Event with ID=trigger1 should exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");

        std::thread::sleep(Duration::from_secs(1));
        es.prune_expired_events();

        // Event with ID=trigger1 should still exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");

        std::thread::sleep(Duration::from_secs(2));
        es.prune_expired_events();

        // Trigger with ID=trigger1
        es.trigger(&Trigger {
            metadata: EventMetadata {
                id: "trigger1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "".into(),
            cameras: Vec::default(),
            pre: Duration::from_secs(1),
            post: Duration::from_secs(2),
        })
        .await;
        es.prune_expired_events();

        // Event with ID=trigger1 should exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");

        std::thread::sleep(Duration::from_secs(1));
        es.prune_expired_events();

        // Event with ID=trigger1 should still exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");

        std::thread::sleep(Duration::from_secs(2));
        es.prune_expired_events();

        // No events should exist
        assert!(es.events.is_empty());
    }

    #[tokio::test]
    async fn test_trigger_3() {
        let store = Arc::new(InMemory::new());
        let mut es = EventSet::new(store, Duration::default()).await;

        // Trigger with ID=trigger1
        es.trigger(&Trigger {
            metadata: EventMetadata {
                id: "trigger1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "".into(),
            cameras: Vec::default(),
            pre: Duration::from_secs(1),
            post: Duration::from_secs(2),
        })
        .await;
        es.prune_expired_events();

        // Event with ID=trigger1 should exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");

        // Trigger with ID=trigger2
        es.trigger(&Trigger {
            metadata: EventMetadata {
                id: "trigger2".into(),
                timestamp: Utc::now().into(),
            },
            reason: "".into(),
            cameras: Vec::default(),
            pre: Duration::from_secs(1),
            post: Duration::from_secs(2),
        })
        .await;
        es.prune_expired_events();

        // Events with ID=trigger1 and ID=trigger2 should exist
        assert_eq!(es.events.len(), 2);
        assert_eq!(es.events[0].metadata.id, "trigger1");
        assert_eq!(es.events[1].metadata.id, "trigger2");

        std::thread::sleep(Duration::from_secs(1));
        es.prune_expired_events();

        // Events with ID=trigger1 and ID=trigger2 should still exist
        assert_eq!(es.events.len(), 2);
        assert_eq!(es.events[0].metadata.id, "trigger1");
        assert_eq!(es.events[1].metadata.id, "trigger2");

        // Trigger with ID=trigger1
        es.trigger(&Trigger {
            metadata: EventMetadata {
                id: "trigger1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "".into(),
            cameras: Vec::default(),
            pre: Duration::from_secs(1),
            post: Duration::from_secs(2),
        })
        .await;
        es.prune_expired_events();

        // Events with ID=trigger1 and ID=trigger2 should still exist
        assert_eq!(es.events.len(), 2);
        assert_eq!(es.events[0].metadata.id, "trigger1");
        assert_eq!(es.events[1].metadata.id, "trigger2");

        std::thread::sleep(Duration::from_secs(1));
        es.prune_expired_events();

        // Events with ID=trigger1 and ID=trigger2 should still exist
        assert_eq!(es.events.len(), 1);
        assert_eq!(es.events[0].metadata.id, "trigger1");

        std::thread::sleep(Duration::from_secs(3));
        es.prune_expired_events();

        // No events should exist
        assert!(es.events.is_empty());
    }

    #[test]
    fn test_update_event_same_trigger() {
        let trigger = Trigger {
            metadata: EventMetadata {
                id: "event1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "Something happened".into(),
            pre: Duration::from_secs(30),
            post: Duration::from_secs(60),
            cameras: Vec::new(),
        };

        let mut event: Event = trigger.clone().into();
        let mut expected = event.clone();

        update_event(&mut event, &trigger);

        expected.reasons = vec![
            EventReason {
                timestamp: trigger.metadata.timestamp,
                reason: "Something happened".into(),
            },
            EventReason {
                timestamp: trigger.metadata.timestamp,
                reason: "Something happened".into(),
            },
        ];

        assert_eq!(event, expected);
    }

    #[test]
    fn test_update_event_start_time() {
        let mut trigger = Trigger {
            metadata: EventMetadata {
                id: "event1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "Something happened".into(),
            pre: Duration::from_secs(30),
            post: Duration::from_secs(60),
            cameras: Vec::new(),
        };

        let mut event: Event = trigger.clone().into();
        let mut expected = event.clone();

        trigger.pre = Duration::from_secs(60);
        expected.start = event.metadata.timestamp - chrono::Duration::try_seconds(60).unwrap();

        expected.reasons = vec![
            EventReason {
                timestamp: trigger.metadata.timestamp,
                reason: "Something happened".into(),
            },
            EventReason {
                timestamp: trigger.metadata.timestamp,
                reason: "Something happened".into(),
            },
        ];

        update_event(&mut event, &trigger);

        assert_eq!(event, expected);
    }

    #[test]
    fn test_update_event_end_time() {
        let mut trigger = Trigger {
            metadata: EventMetadata {
                id: "event1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "Something happened".into(),
            pre: Duration::from_secs(30),
            post: Duration::from_secs(60),
            cameras: Vec::new(),
        };

        let mut event: Event = trigger.clone().into();
        let mut expected = event.clone();

        trigger.post = Duration::from_secs(120);
        expected.end = event.metadata.timestamp + chrono::Duration::try_seconds(120).unwrap();

        expected.reasons = vec![
            EventReason {
                timestamp: trigger.metadata.timestamp,
                reason: "Something happened".into(),
            },
            EventReason {
                timestamp: trigger.metadata.timestamp,
                reason: "Something happened".into(),
            },
        ];

        update_event(&mut event, &trigger);

        assert_eq!(event, expected);
    }

    #[test]
    fn test_update_event_end_time_time_passing() {
        let mut trigger = Trigger {
            metadata: EventMetadata {
                id: "event1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "Something happened".into(),
            pre: Duration::from_secs(30),
            post: Duration::from_secs(60),
            cameras: Vec::new(),
        };

        let mut event: Event = trigger.clone().into();
        let mut expected = event.clone();

        let reason_1_timestamp = trigger.metadata.timestamp;

        trigger.metadata.timestamp += chrono::Duration::try_seconds(1).unwrap();
        trigger.reason = "Something else happened".into();
        expected.end += chrono::Duration::try_seconds(1).unwrap();

        let reason_2_timestamp = trigger.metadata.timestamp;

        expected.reasons = vec![
            EventReason {
                timestamp: reason_1_timestamp,
                reason: "Something happened".into(),
            },
            EventReason {
                timestamp: reason_2_timestamp,
                reason: "Something else happened".into(),
            },
        ];

        update_event(&mut event, &trigger);

        assert_eq!(event, expected);
    }

    #[test]
    fn test_update_event_new_cameras_1() {
        let mut trigger = Trigger {
            metadata: EventMetadata {
                id: "event1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "Something happened".into(),
            pre: Duration::from_secs(30),
            post: Duration::from_secs(60),
            cameras: vec!["camera-1".into()],
        };

        let mut event: Event = trigger.clone().into();

        trigger.cameras = vec!["camera-1".into(), "camera-2".into()];

        update_event(&mut event, &trigger);

        assert_eq!(
            event
                .cameras
                .iter()
                .map(|c| c.name.clone())
                .collect::<Vec<String>>(),
            vec!["camera-1".to_string(), "camera-2".to_string()]
        );
    }

    #[test]
    fn test_update_event_new_cameras_2() {
        let mut trigger = Trigger {
            metadata: EventMetadata {
                id: "event1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "Something happened".into(),
            pre: Duration::from_secs(30),
            post: Duration::from_secs(60),
            cameras: vec!["camera-1".into()],
        };

        let mut event: Event = trigger.clone().into();

        trigger.cameras = vec!["camera-2".into()];

        update_event(&mut event, &trigger);

        assert_eq!(
            event
                .cameras
                .iter()
                .map(|c| c.name.clone())
                .collect::<Vec<String>>(),
            vec!["camera-1".to_string(), "camera-2".to_string()]
        );
    }
}
