use crate::{hls_client::HlsClient, segments::Playlist};
use miette::IntoDiagnostic;
use satori_common::{
    ArchiveCommand, ArchiveSegmentsCommand, CameraSegments, Event, EventReason, Trigger,
};
use std::{
    collections::VecDeque,
    fs::File,
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::{error, info, warn};
use url::Url;

#[derive(Clone, Debug)]
struct PendingArchiveOperation {
    command: ArchiveCommand,
    retry_count: u32,
}

const MAX_RETRIES: u32 = 3;

#[derive(Default)]
pub(crate) struct EventSet {
    events: Vec<Event>,
    pending_operations: VecDeque<PendingArchiveOperation>,

    event_ttl: Duration,
    backing_file_name: PathBuf,
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
            pending_operations: VecDeque::new(),
            event_ttl,
            backing_file_name: path.into(),
        }
    }

    #[tracing::instrument]
    fn load(path: &Path) -> miette::Result<Vec<Event>> {
        let file = File::open(path).into_diagnostic()?;
        serde_json::from_reader(&file).into_diagnostic()
    }

    #[tracing::instrument(skip_all)]
    fn save(&self) -> miette::Result<()> {
        let file = File::create(&self.backing_file_name).into_diagnostic()?;
        serde_json::to_writer(&file, &self.events).into_diagnostic()
    }

    #[tracing::instrument(skip_all)]
    fn attempt_save(&self) {
        if let Err(err) = self.save() {
            error!(
                "Could not persist event list file {}, reason: {}. Active events will be lost upon restart.",
                self.backing_file_name.display(),
                err
            );
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn trigger(&mut self, trigger: &Trigger) {
        metrics::counter!(
            crate::METRIC_TRIGGERS,
            "id" => trigger.metadata.id.clone()
        )
        .increment(1);

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
        http_client: &reqwest::Client,
        archiver_url: &Url,
    ) {
        // Process pending operations first
        self.process_pending_operations(http_client, archiver_url)
            .await;

        // Do nothing if there are no events in the queue
        if self.events.is_empty() {
            return;
        }

        // Collect archive commands to send
        let mut commands_to_send = Vec::new();

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
                    // Queue archive command for segments
                    let cmd = ArchiveCommand::Segments(ArchiveSegmentsCommand {
                        camera_name: camera.name.clone(),
                        camera_url: camera_client.get_camera_url(&camera.name).unwrap(),
                        segment_list: new_segments.clone(),
                    });
                    commands_to_send.push(cmd);
                }

                // Update segment list in event
                camera.segment_list.append(&mut new_segments);
            }

            // Queue archive command for event metadata
            commands_to_send.push(ArchiveCommand::EventMetadata(event.clone()));
        }

        // Send all commands
        for cmd in commands_to_send {
            self.queue_archive_command(cmd, http_client, archiver_url)
                .await;
        }

        // Now remove any events that have outlived the TTL
        self.prune_expired_events();

        metrics::gauge!(crate::METRIC_ACTIVE_EVENTS).set(self.events.len() as f64);

        self.attempt_save();
    }

    #[tracing::instrument(skip_all)]
    async fn queue_archive_command(
        &mut self,
        cmd: ArchiveCommand,
        http_client: &reqwest::Client,
        archiver_url: &Url,
    ) {
        match self.send_archive_command(&cmd, http_client, archiver_url).await {
            Ok(()) => {
                info!("Successfully sent archive command");
            }
            Err(e) => {
                error!("Failed to send archive command: {}", e);
                // Add to pending operations for retry
                self.pending_operations.push_back(PendingArchiveOperation {
                    command: cmd,
                    retry_count: 0,
                });
            }
        }
    }

    #[tracing::instrument(skip_all)]
    async fn process_pending_operations(
        &mut self,
        http_client: &reqwest::Client,
        archiver_url: &Url,
    ) {
        let mut remaining_operations = VecDeque::new();

        while let Some(mut op) = self.pending_operations.pop_front() {
            match self.send_archive_command(&op.command, http_client, archiver_url).await {
                Ok(()) => {
                    info!("Successfully sent pending archive command");
                }
                Err(e) => {
                    op.retry_count += 1;
                    if op.retry_count < MAX_RETRIES {
                        error!(
                            "Failed to send pending archive command (retry {}/{}): {}",
                            op.retry_count, MAX_RETRIES, e
                        );
                        remaining_operations.push_back(op);
                    } else {
                        error!(
                            "Failed to send pending archive command after {} retries, dropping: {}",
                            MAX_RETRIES, e
                        );
                    }
                }
            }
        }

        self.pending_operations = remaining_operations;
    }

    #[tracing::instrument(skip_all)]
    async fn send_archive_command(
        &self,
        cmd: &ArchiveCommand,
        http_client: &reqwest::Client,
        archiver_url: &Url,
    ) -> Result<(), String> {
        let url = archiver_url.join("/archive").map_err(|e| e.to_string())?;

        let response = http_client
            .post(url)
            .json(cmd)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!("HTTP request failed with status: {}", response.status()))
        }
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
                    metrics::counter!(
                        crate::METRIC_EXPIRED_EVENTS,
                        "id" => event.metadata.id.clone()
                    )
                    .increment(1);
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
