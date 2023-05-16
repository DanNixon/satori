use crate::{error::EventProcessorResult, hls_client::HlsClient, segments::Playlist};
use kagiyama::prometheus::registry::Registry;
use satori_common::{ArchiveCommand, CameraSegments, Event, EventReason, Message, Trigger};
use std::{
    fs::File,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::{error, info, warn};

#[derive(Default)]
pub(crate) struct EventSet {
    events: Vec<Event>,

    event_ttl: Duration,
    backing_file_name: PathBuf,

    triggers: metrics::TriggersMetric,
    active_events: metrics::ActiveEventsMetric,
    expired_events: metrics::ExpiredEventsMetric,
}

impl EventSet {
    #[tracing::instrument]
    pub(crate) fn load_or_new(path: &Path, event_ttl: Duration) -> Self {
        Self {
            // Try and load active events from disk
            events: match Self::load(path) {
                Ok(v) => v,
                Err(err) => {
                    // Otherwise provide an event set
                    warn!(
                        "Failed to read event set file {}, reason: {}",
                        path.display(),
                        err
                    );
                    Default::default()
                }
            },
            event_ttl,
            backing_file_name: path.into(),
            triggers: Default::default(),
            active_events: Default::default(),
            expired_events: Default::default(),
        }
    }

    #[tracing::instrument]
    fn load(path: &Path) -> EventProcessorResult<Vec<Event>> {
        let file = File::open(path)?;
        Ok(serde_json::from_reader(&file)?)
    }

    #[tracing::instrument(skip_all)]
    fn save(&self) -> EventProcessorResult<()> {
        let file = File::create(&self.backing_file_name)?;
        Ok(serde_json::to_writer(&file, &self.events)?)
    }

    #[tracing::instrument(skip_all)]
    fn attempt_save(&self) {
        if let Err(err) = self.save() {
            error!(
                "Could not persist event list file {}, reason: {}. Active events will be lost upon restart.",
                self.backing_file_name.display(), err
            );
        }
    }

    pub(crate) fn register_metrics(&self, registry: &mut Registry) {
        registry.register("triggers", "Trigger count", self.triggers.clone());

        registry.register(
            "active_events",
            "Number of active events",
            self.active_events.clone(),
        );

        registry.register(
            "expired_events",
            "Processed events count",
            self.expired_events.clone(),
        );
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn trigger(&mut self, trigger: &Trigger) {
        self.triggers
            .get_or_create(&metrics::Labels::new(&trigger.metadata.id))
            .inc();

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

        self.attempt_save();
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn process(
        &mut self,
        camera_client: &HlsClient,
        mqtt_client: &mqtt_channel_client::Client,
        mqtt_control_topic: &str,
    ) {
        // Do nothing if there are no events in the queue
        if self.events.is_empty() {
            return;
        }

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
                let mut new_segments: Vec<PathBuf> = segments
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
                    // Send archive command for segments
                    if let Err(err) = satori_common::mqtt::send_json(
                        mqtt_client,
                        mqtt_control_topic,
                        &Message::ArchiveCommand(ArchiveCommand::Segments(CameraSegments {
                            name: camera.name.clone(),
                            segment_list: new_segments.clone(),
                        })),
                    ) {
                        error!("Failed to send archive segments command, reason: {}", err);
                    }
                }

                // Update segment list in event
                camera.segment_list.append(&mut new_segments);
            }

            // Send archive command for event
            if let Err(err) = satori_common::mqtt::send_json(
                mqtt_client,
                mqtt_control_topic,
                &Message::ArchiveCommand(ArchiveCommand::EventMetadata(event.clone())),
            ) {
                error!("Failed to send archive event command, reason: {}", err);
            }
        }

        // Now remove any events that have outlived the TTL
        self.prune_expired_events();

        self.active_events.set(self.events.len() as i64);

        self.attempt_save();
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
                    self.expired_events
                        .get_or_create(&metrics::Labels::new(&event.metadata.id))
                        .inc();
                    None
                } else {
                    Some(event.clone())
                }
            })
            .collect();

        info!("{} event(s) remain", self.events.len());
    }
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

mod metrics {
    use kagiyama::prometheus::{
        self as prometheus_client,
        encoding::EncodeLabelSet,
        metrics::{counter::Counter, family::Family, gauge::Gauge},
    };

    pub(super) type TriggersMetric = Family<Labels, Counter>;
    pub(super) type ActiveEventsMetric = Gauge;
    pub(super) type ExpiredEventsMetric = Family<Labels, Counter>;

    #[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
    pub(super) struct Labels {
        id: String,
    }

    impl Labels {
        pub(super) fn new(id: &str) -> Self {
            Self { id: id.to_owned() }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::Utc;
    use satori_common::EventMetadata;

    #[test]
    fn test_load_bad_file_gives_empty_event_set() {
        let es = EventSet::load_or_new(
            &std::env::temp_dir().join("not_a_real_file.json"),
            Duration::default(),
        );
        assert!(es.events.is_empty());
    }

    #[test]
    fn test_trigger_1() {
        let mut es = EventSet::default();

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
        });
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
        });
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

    #[test]
    fn test_trigger_2() {
        let mut es = EventSet::default();

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
        });
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
        });
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

    #[test]
    fn test_trigger_3() {
        let mut es = EventSet::default();

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
        });
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
        });
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
        });
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
        expected.start = event.metadata.timestamp - chrono::Duration::seconds(60);

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
        expected.end = event.metadata.timestamp + chrono::Duration::seconds(120);

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

        trigger.metadata.timestamp += chrono::Duration::seconds(1);
        trigger.reason = "Something else happened".into();
        expected.end += chrono::Duration::seconds(1);

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
