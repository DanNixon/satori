use crate::{AppContext, task::ArchiveTask};
use miette::IntoDiagnostic;
use satori_common::{ArchiveCommand, ArchiveSegmentsCommand, Event};
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
    fn load(path: &Path) -> miette::Result<Self> {
        let file = File::open(path).into_diagnostic()?;
        let queue = Self {
            queue: serde_json::from_reader(file).into_diagnostic()?,
            backing_file_name: path.into(),
        };
        queue.update_queue_length_metrics();
        Ok(queue)
    }

    #[tracing::instrument(skip_all)]
    fn save(&self) -> miette::Result<()> {
        info!("Saving job queue to {}", self.backing_file_name.display());
        let file = File::create(&self.backing_file_name).into_diagnostic()?;
        serde_json::to_writer(file, &self.queue).into_diagnostic()
    }

    #[tracing::instrument(skip_all)]
    fn attempt_save(&self) {
        if let Err(err) = self.save() {
            error!(
                "Could not persist queue file {}, reason: {}. Pending tasks in the queue will be lost upon restart.",
                self.backing_file_name.display(),
                err
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
            "type" => "event"
        )
        .set(event_queue_length);

        let segment_queue_length = self
            .queue
            .iter()
            .filter(|t| matches!(t, ArchiveTask::CameraSegment(_)))
            .count() as f64;

        metrics::gauge!(
            crate::METRIC_QUEUE_LENGTH,
            "type" => "segment"
        )
        .set(segment_queue_length);
    }

    #[tracing::instrument(skip_all)]
    pub(crate) fn add_archive_command(&mut self, cmd: ArchiveCommand) {
        match cmd {
            ArchiveCommand::EventMetadata(event) => {
                self.add_event_metadata(event);
            }
            ArchiveCommand::Segments(cmd) => {
                self.add_segments(cmd);
            }
        }
        info!("Task queue length is now: {}", self.queue.len());
    }

    #[tracing::instrument(skip_all)]
    fn add_event_metadata(&mut self, event: Event) {
        info!("Queueing archive event metadata command");
        self.queue.push_back(ArchiveTask::EventMetadata(event));

        self.attempt_save();
        self.update_queue_length_metrics();
    }

    #[tracing::instrument(skip_all)]
    fn add_segments(&mut self, msg: ArchiveSegmentsCommand) {
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

        self.attempt_save();
        self.update_queue_length_metrics();
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn process_one(&mut self, context: &AppContext) {
        if let Some(task) = self.queue.front() {
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
                "type" => task_type,
                "result" => task_result
            )
            .increment(1);

            match result {
                Ok(()) => {
                    info!("Successfully processed task: {:?}", task);

                    // Remove the task from the queue
                    let _ = self.queue.pop_front();
                    self.attempt_save();
                    self.update_queue_length_metrics();
                }
                Err(err) => {
                    error!("Failed to process task: {:?}, reason: {}", task, err);
                }
            }
        }
    }

    /// Process a task synchronously (for HTTP endpoint)
    #[tracing::instrument(skip_all)]
    pub(crate) async fn process_task_sync(
        &mut self,
        task: ArchiveTask,
        context: &AppContext,
    ) -> miette::Result<()> {
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
            "type" => task_type,
            "result" => task_result
        )
        .increment(1);

        match &result {
            Ok(()) => {
                info!("Successfully processed task: {:?}", task);
            }
            Err(err) => {
                error!("Failed to process task: {:?}, reason: {}", task, err);
            }
        }

        result
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use satori_common::{ArchiveCommand, ArchiveSegmentsCommand};
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

        let cmd = ArchiveCommand::Segments(ArchiveSegmentsCommand {
            camera_name: "camera-1".into(),
            camera_url: Url::parse("http://localhost:8080/stream.m3u8").unwrap(),
            segment_list: vec![],
        });
        queue.add_archive_command(cmd);
        assert!(queue.queue.is_empty());
    }

    #[test]
    fn test_archive_segments_some_segments_some_tasks() {
        let mut queue = ArchiveTaskQueue::default();
        assert!(queue.queue.is_empty());

        let cmd = ArchiveCommand::Segments(ArchiveSegmentsCommand {
            camera_name: "camera-1".into(),
            camera_url: Url::parse("http://localhost:8080/stream.m3u8").unwrap(),
            segment_list: vec!["one.ts".into(), "two.ts".into()],
        });
        queue.add_archive_command(cmd);
        assert_eq!(queue.queue.len(), 2);
    }
}
