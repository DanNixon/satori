use crate::{error::ArchiverResult, task::ArchiveTask, Context};
use futures::StreamExt;
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

    fn update_queue_length_metrics(&self) {
        let event_queue_length = self
            .queue
            .iter()
            .filter(|t| matches!(t, ArchiveTask::EventMetadata(_)))
            .count() as f64;

        metrics::gauge!(
            crate::METRIC_QUEUE_LENGTH,
            event_queue_length,
            "type" => "event"
        );

        let segment_queue_length = self
            .queue
            .iter()
            .filter(|t| matches!(t, ArchiveTask::CameraSegment(_)))
            .count() as f64;

        metrics::gauge!(
            crate::METRIC_QUEUE_LENGTH,
            segment_queue_length,
            "type" => "segment"
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
                .filter_map(|task| async move {
                    let task_type = match &task {
                        ArchiveTask::EventMetadata(_) => "event",
                        ArchiveTask::CameraSegment(_) => "segment",
                    };

                    let result = task.run(context).await;

                    let task_result = match &result {
                        Ok(_) => "success",
                        Err(_) => "failure",
                    };

                    metrics::counter!(
                        crate::METRIC_PROCESSED_TASKS,
                        1,
                        "type" => task_type,
                        "result" => task_result
                    );

                    match result {
                        Ok(()) => None,
                        Err(err) => {
                            error!("Failed to process task: {:?}, reason: {}", task, err);
                            Some(task)
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

#[cfg(test)]
mod test {
    use super::*;
    use rumqttc::{Publish, QoS};
    use satori_common::{ArchiveCommand, ArchiveSegmentsCommand, Message};
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
}
