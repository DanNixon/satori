use crate::{error::ArchiverResult, task::ArchiveTask, Context};
use futures::StreamExt;
use kagiyama::prometheus::registry::Registry;
use satori_common::{mqtt::PublishExt, ArchiveCommand, ArchiveSegmentsCommand, Event};
use std::{
    collections::VecDeque,
    fs::File,
    path::{Path, PathBuf},
};
use tracing::{debug, error, info, warn};

#[derive(Default)]
pub(crate) struct ArchiveTaskQueue {
    queue: VecDeque<ArchiveTask>,

    backing_file_name: PathBuf,

    queue_length: metrics::QueuedTaskMetric,
    finished_tasks: metrics::FinishedTaskMetric,
}

impl ArchiveTaskQueue {
    #[tracing::instrument]
    pub(crate) fn load_or_new(path: &Path) -> Self {
        // Try and load the queue from disk
        match Self::load(path) {
            Ok(i) => i,
            Err(err) => {
                warn!(
                    "Failed to read queue file {}, reason: {}",
                    path.display(),
                    err
                );
                // Otherwise provide an empty queue
                let queue = Self {
                    queue: Default::default(),
                    backing_file_name: path.into(),
                    queue_length: Default::default(),
                    finished_tasks: Default::default(),
                };
                queue.update_queue_length_metrics();
                queue
            }
        }
    }

    #[tracing::instrument]
    fn load(path: &Path) -> ArchiverResult<Self> {
        let file = File::open(path)?;
        let queue = Self {
            queue: serde_json::from_reader(file)?,
            backing_file_name: path.into(),
            queue_length: Default::default(),
            finished_tasks: Default::default(),
        };
        queue.update_queue_length_metrics();
        Ok(queue)
    }

    #[tracing::instrument(skip_all)]
    fn save(&self) -> ArchiverResult<()> {
        info!("Saving job queue to {}", self.backing_file_name.display());
        let file = File::create(&self.backing_file_name)?;
        Ok(serde_json::to_writer(file, &self.queue)?)
    }

    #[tracing::instrument(skip_all)]
    fn attempt_save(&self) {
        if let Err(err) = self.save() {
            error!(
                "Could not persist queue file {}, reason: {}. Pending tasks in the queue will be lost upon restart.",
                self.backing_file_name.display(), err
            );
        }
    }

    pub(crate) fn register_metrics(&self, registry: &mut Registry) {
        registry.register(
            "queue_length",
            "Number of tasks in queue",
            self.queue_length.clone(),
        );

        registry.register(
            "finished_tasks",
            "Finished task count",
            self.finished_tasks.clone(),
        );
    }

    fn update_queue_length_metrics(&self) {
        self.queue_length
            .get_or_create(&metrics::QueuedTaskLabels::event())
            .set(
                self.queue
                    .iter()
                    .filter(|t| matches!(t, ArchiveTask::EventMetadata(_)))
                    .count() as i64,
            );

        self.queue_length
            .get_or_create(&metrics::QueuedTaskLabels::segment())
            .set(
                self.queue
                    .iter()
                    .filter(|t| matches!(t, ArchiveTask::CameraSegment(_)))
                    .count() as i64,
            );
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn handle_mqtt_message(&mut self, msg: rumqttc::Publish) {
        match msg.try_payload_from_json::<satori_common::Message>() {
            Ok(msg) => {
                if let satori_common::Message::ArchiveCommand(cmd) = msg {
                    match cmd {
                        ArchiveCommand::EventMetadata(cmd) => {
                            self.handle_archive_event_metadata_message(cmd);
                        }
                        ArchiveCommand::Segments(cmd) => {
                            self.handle_archive_segments_message(cmd);
                        }
                    }
                    info!("Task queue length is now: {}", self.queue.len());
                }
            }
            Err(e) => {
                error!("Failed to parse message, error={}", e);
            }
        }
    }

    #[tracing::instrument(skip_all)]
    fn handle_archive_event_metadata_message(&mut self, event: Event) {
        info!("Queueing archive event metadata command");
        self.queue.push_back(ArchiveTask::EventMetadata(event));

        self.update_queue_length_metrics();

        self.attempt_save();
    }

    #[tracing::instrument(skip_all)]
    fn handle_archive_segments_message(&mut self, msg: ArchiveSegmentsCommand) {
        info!("Queueing archive video segments command");
        for segment in msg.segment_list {
            debug!("Adding video segment to queue: {}", segment.display());
            self.queue
                .push_back(ArchiveTask::CameraSegment(crate::task::CameraSegment {
                    camera_name: msg.camera_name.clone(),
                    camera_url: msg.camera_url.clone(),
                    filename: segment,
                }));
        }

        self.update_queue_length_metrics();

        self.attempt_save();
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn process(&mut self, context: &Context) {
        if !self.queue.is_empty() {
            self.queue = futures::stream::iter(self.queue.clone().into_iter())
                .filter_map(|task| {
                    let finished_tasks = self.finished_tasks.clone();
                    async move {
                        let task_kind = match &task {
                            ArchiveTask::EventMetadata(_) => metrics::TaskKind::Event,
                            ArchiveTask::CameraSegment(_) => metrics::TaskKind::Segment,
                        };

                        match task.run(context).await {
                            Ok(()) => {
                                finished_tasks
                                    .get_or_create(&metrics::FinishedTaskLabels::success(task_kind))
                                    .inc();
                                None
                            }
                            Err(err) => {
                                error!("Failed to process task: {:?}, reason: {}", task, err);
                                finished_tasks
                                    .get_or_create(&metrics::FinishedTaskLabels::failure(task_kind))
                                    .inc();
                                Some(task)
                            }
                        }
                    }
                })
                .collect()
                .await;

            self.attempt_save();
        }

        self.update_queue_length_metrics();
    }
}

mod metrics {
    use kagiyama::prometheus::{
        self as prometheus_client,
        encoding::{EncodeLabelSet, EncodeLabelValue},
        metrics::{counter::Counter, family::Family, gauge::Gauge},
    };

    pub(super) type QueuedTaskMetric = Family<QueuedTaskLabels, Gauge>;
    pub(super) type FinishedTaskMetric = Family<FinishedTaskLabels, Counter>;

    #[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelValue)]
    pub(super) enum TaskKind {
        Event,
        Segment,
    }

    #[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelValue)]
    pub(super) enum TaskResult {
        Success,
        Failure,
    }

    #[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
    pub(super) struct QueuedTaskLabels {
        kind: TaskKind,
    }

    impl QueuedTaskLabels {
        pub(super) fn event() -> Self {
            Self {
                kind: TaskKind::Event,
            }
        }

        pub(super) fn segment() -> Self {
            Self {
                kind: TaskKind::Segment,
            }
        }
    }

    #[derive(Debug, Clone, Hash, PartialEq, Eq, EncodeLabelSet)]
    pub(super) struct FinishedTaskLabels {
        kind: TaskKind,
        result: TaskResult,
    }

    impl FinishedTaskLabels {
        pub(super) fn success(kind: TaskKind) -> Self {
            Self {
                kind,
                result: TaskResult::Success,
            }
        }

        pub(super) fn failure(kind: TaskKind) -> Self {
            Self {
                kind,
                result: TaskResult::Failure,
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::Utc;
    use rumqttc::{Publish, QoS};
    use satori_common::{ArchiveCommand, ArchiveSegmentsCommand, EventMetadata, Message};
    use url::Url;

    #[test]
    fn test_load_bad_file_gives_empty_queue() {
        let queue =
            ArchiveTaskQueue::load_or_new(&std::env::temp_dir().join("not_a_real_file.json"));
        assert!(queue.queue.is_empty());
    }

    #[test]
    fn test_archive_segments_no_segments_no_tasks() {
        let mut queue = ArchiveTaskQueue::default();
        assert!(queue.queue.is_empty());

        let msg = Message::ArchiveCommand(ArchiveCommand::Segments(ArchiveSegmentsCommand {
            camera_name: "camera-1".into(),
            camera_url: Url::parse("http://localhost:8080/stream.m3u8").unwrap(),
            segment_list: vec![],
        }));
        let msg = Publish::new("", QoS::ExactlyOnce, serde_json::to_string(&msg).unwrap());
        queue.handle_mqtt_message(msg);
        assert!(queue.queue.is_empty());
    }

    #[test]
    fn test_archive_segments_some_segments_some_tasks() {
        let mut queue = ArchiveTaskQueue::default();
        assert!(queue.queue.is_empty());

        let msg = Message::ArchiveCommand(ArchiveCommand::Segments(ArchiveSegmentsCommand {
            camera_name: "camera-1".into(),
            camera_url: Url::parse("http://localhost:8080/stream.m3u8").unwrap(),
            segment_list: vec!["one.ts".into(), "two.ts".into()],
        }));
        let msg = Publish::new("", QoS::ExactlyOnce, serde_json::to_string(&msg).unwrap());
        queue.handle_mqtt_message(msg);
        assert_eq!(queue.queue.len(), 2);
    }

    #[test]
    fn test_queue_length_metric_init() {
        let queue = ArchiveTaskQueue::default();
        assert_eq!(
            queue
                .queue_length
                .get_or_create(&metrics::QueuedTaskLabels::event())
                .get(),
            0
        );
        assert_eq!(
            queue
                .queue_length
                .get_or_create(&metrics::QueuedTaskLabels::segment())
                .get(),
            0
        );
    }

    #[test]
    fn test_queue_length_metric() {
        let mut queue = ArchiveTaskQueue::default();

        // Add an event to the queue
        let msg = Message::ArchiveCommand(ArchiveCommand::EventMetadata(Event {
            metadata: EventMetadata {
                id: "test-1".into(),
                timestamp: Utc::now().into(),
            },
            start: Utc::now().into(),
            end: Utc::now().into(),
            reasons: Default::default(),
            cameras: Default::default(),
        }));
        let msg = Publish::new("", QoS::ExactlyOnce, serde_json::to_string(&msg).unwrap());
        queue.handle_mqtt_message(msg);

        // Add two segments to the queue
        let msg = Message::ArchiveCommand(ArchiveCommand::Segments(ArchiveSegmentsCommand {
            camera_name: "camera-1".into(),
            camera_url: Url::parse("http://localhost:8080/stream.m3u8").unwrap(),
            segment_list: vec!["one.ts".into(), "two.ts".into()],
        }));
        let msg = Publish::new("", QoS::ExactlyOnce, serde_json::to_string(&msg).unwrap());
        queue.handle_mqtt_message(msg);

        assert_eq!(
            queue
                .queue_length
                .get_or_create(&metrics::QueuedTaskLabels::event())
                .get(),
            1
        );
        assert_eq!(
            queue
                .queue_length
                .get_or_create(&metrics::QueuedTaskLabels::segment())
                .get(),
            2
        );
    }
}
