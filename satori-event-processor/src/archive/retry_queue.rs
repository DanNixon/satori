use super::tasks::ArchiveTask;
use crate::{archive::tasks::ArchiveOperation, o11y::ArchiveTaskResult};
use chrono::{Duration, Utc};
use miette::{Context, IntoDiagnostic};
use object_store::{ObjectStore, ObjectStoreExt, path::Path};
use std::sync::Arc;
use tracing::{debug, info, warn};

type Queue = Vec<ArchiveTask>;

pub(crate) struct ArchiveRetryQueue {
    /// The state store
    store: Arc<dyn ObjectStore>,
    /// Amount of time to keep failed tasks in the queue
    task_ttl: Duration,

    /// Queue of failed tasks to retry
    queue: Queue,
}

impl ArchiveRetryQueue {
    pub(crate) async fn new(store: Arc<dyn ObjectStore>, task_ttl: Duration) -> Self {
        let queue = match load_queue_from_store(store.clone()).await {
            Ok(q) => q,
            Err(e) => {
                warn!("Failed to load archive retry queue from store: {e}");
                Queue::new()
            }
        };

        Self {
            store,
            task_ttl,
            queue,
        }
    }

    pub(crate) async fn save(&self) {
        if let Err(e) = save_queue_to_store(self.store.clone(), &self.queue).await {
            warn!("Failed to save archive retry queue to store: {e}");
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn push(&mut self, task: ArchiveTask) {
        // When adding an event to the queue, remove any older instances that match it.
        // Ensuring that the most recent version of the event is archived.
        // Remove any older instances that match the event.
        if let ArchiveOperation::Event(event) = &task.op {
            // Find an event with the same metadata already in the queue.
            // There should only ever be one matching event existing in the queue.
            let existing = self
                .queue
                .iter()
                .find(|t| {
                    if let ArchiveOperation::Event(ee) = &t.op {
                        ee.metadata == event.metadata
                    } else {
                        false
                    }
                })
                .cloned();

            match existing {
                Some(existing) => {
                    if existing.birth > task.birth {
                        warn!(
                            "Discarding {task:?} as it appears to be an older description than {existing:?} already in the queue"
                        );
                        // Do nothing with the new task
                    } else {
                        // Remove the existing task from the queue
                        self.queue.retain(|t| *t != existing);

                        // Add the newer event to the queue
                        debug!("Pushing {task:?} to queue, replacing older task {existing:?}");
                        self.queue.push(task);
                    }
                }
                None => {
                    debug!("Pushing {task:?} to the queue");
                    self.queue.push(task);
                }
            }
        } else {
            debug!("Pushing {task:?} to the queue");
            self.queue.push(task);
        }
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn process(
        &mut self,
        task_tx: &tokio::sync::mpsc::UnboundedSender<ArchiveTask>,
    ) -> miette::Result<()> {
        // Save the queue before doing anything
        self.save().await;

        // Prune old tasks
        prune_old_tasks(&mut self.queue, self.task_ttl);

        // Resubmit remaining failed tasks
        while let Some(task) = self.queue.pop() {
            task_tx
                .send(task)
                .into_diagnostic()
                .wrap_err("Failed to transmit archive task on channel")?;
        }

        // Save the queue after processing
        self.save().await;

        metrics::gauge!(crate::o11y::ARCHIVE_RETRY_QUEUE_LENGTH).set(self.queue.len() as f64);

        Ok(())
    }
}

const STORE_PATH: &str = "archive_retry_queue.json";

async fn load_queue_from_store(store: Arc<dyn ObjectStore>) -> miette::Result<Queue> {
    let state_file = store.get(&Path::from(STORE_PATH)).await.into_diagnostic()?;
    let state_data = state_file.bytes().await.into_diagnostic()?;
    let queue: Queue = serde_json::from_slice(&state_data).into_diagnostic()?;
    info!("Loaded queue with {} entries from store", queue.len());
    Ok(queue)
}

async fn save_queue_to_store(store: Arc<dyn ObjectStore>, queue: &Queue) -> miette::Result<()> {
    let bytes = serde_json::to_vec(queue).into_diagnostic()?;
    let _ = store
        .put(&Path::from(STORE_PATH), bytes.into())
        .await
        .into_diagnostic()?;
    info!("Saved queue to store");
    Ok(())
}

#[tracing::instrument(skip(queue))]
fn prune_old_tasks(queue: &mut Queue, age: Duration) {
    let deadline = Utc::now() - age;

    queue.retain(|task| {
        let keep = task.birth >= deadline;

        if !keep {
            crate::o11y::inc_archive_task_metric(&task.api_url, ArchiveTaskResult::FailureExpired);
        }

        info!(
            "{} task: {task:?}",
            if keep { "Keeping" } else { "Discarding" }
        );

        keep
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tasks::ArchiveOperation;
    use chrono::{Duration, Utc};
    use object_store::memory::InMemory;
    use satori_common::{Event, EventMetadata, EventReason};
    use std::sync::Arc;
    use url::Url;

    #[tokio::test]
    async fn new_empty_queue() {
        let store = Arc::new(InMemory::new());
        let task_ttl = Duration::hours(1);
        let queue = ArchiveRetryQueue::new(store, task_ttl).await;
        assert!(queue.queue.is_empty());
    }

    #[tokio::test]
    async fn push_task() {
        let store = Arc::new(InMemory::new());
        let task_ttl = Duration::hours(1);
        let mut queue = ArchiveRetryQueue::new(store, task_ttl).await;

        let task = ArchiveTask {
            birth: Utc::now(),
            api_url: Url::parse("http://localhost").unwrap(),
            op: ArchiveOperation::Segment {
                camera_name: "noop".to_owned(),
                url: Url::parse("http://localhost").unwrap(),
            },
        };
        queue.push(task);
        assert_eq!(queue.queue.len(), 1);
    }

    #[tokio::test]
    async fn save_and_load() {
        let store = Arc::new(InMemory::new());

        let task = ArchiveTask {
            birth: Utc::now(),
            api_url: Url::parse("http://localhost").unwrap(),
            op: ArchiveOperation::Segment {
                camera_name: "noop".to_owned(),
                url: Url::parse("http://localhost").unwrap(),
            },
        };

        let queue_data = vec![task];

        save_queue_to_store(store.clone(), &queue_data)
            .await
            .unwrap();

        let loaded = load_queue_from_store(store).await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded, queue_data);
    }

    #[test]
    fn prune() {
        let task1 = ArchiveTask {
            birth: Utc::now() - Duration::hours(2),
            api_url: Url::parse("http://localhost").unwrap(),
            op: ArchiveOperation::Segment {
                camera_name: "noop".to_owned(),
                url: Url::parse("http://localhost").unwrap(),
            },
        };
        let task2 = ArchiveTask {
            birth: Utc::now() - Duration::minutes(30),
            api_url: Url::parse("http://localhost").unwrap(),
            op: ArchiveOperation::Segment {
                camera_name: "noop".to_owned(),
                url: Url::parse("http://localhost").unwrap(),
            },
        };

        let mut queue = vec![task1, task2.clone()];

        let age = Duration::hours(1);
        prune_old_tasks(&mut queue, age);
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0], task2);
    }

    #[tokio::test]
    async fn process_queue() {
        let store = Arc::new(InMemory::new());
        let task_ttl = Duration::hours(1);

        let mut queue = ArchiveRetryQueue::new(store, task_ttl).await;
        let task = ArchiveTask {
            birth: Utc::now(),
            api_url: Url::parse("http://localhost").unwrap(),
            op: ArchiveOperation::Segment {
                camera_name: "noop".to_owned(),
                url: Url::parse("http://localhost").unwrap(),
            },
        };
        queue.push(task.clone());

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        queue.process(&tx).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.birth, task.birth);
        assert!(queue.queue.is_empty());
    }

    #[tokio::test]
    async fn newer_event_removes_older_instances() {
        let store = Arc::new(InMemory::new());
        let task_ttl = Duration::hours(1);

        let mut queue = ArchiveRetryQueue::new(store, task_ttl).await;

        let ts = (Utc::now() - Duration::seconds(10)).into();

        let task1 = ArchiveTask {
            birth: Utc::now() - Duration::seconds(5),
            api_url: "http://localhost".parse().unwrap(),
            op: ArchiveOperation::Event(Event {
                metadata: EventMetadata {
                    id: "test".to_owned(),
                    timestamp: ts,
                },
                reasons: vec![EventReason {
                    timestamp: Utc::now().into(),
                    reason: "aaa".to_owned(),
                }],
                start: Utc::now().into(),
                end: Utc::now().into(),
                cameras: Default::default(),
            }),
        };

        let other_task = ArchiveTask {
            birth: Utc::now() - Duration::seconds(5),
            api_url: "http://localhost".parse().unwrap(),
            op: ArchiveOperation::Event(Event {
                metadata: EventMetadata {
                    id: "not a test".to_owned(),
                    timestamp: ts,
                },
                reasons: Default::default(),
                start: Utc::now().into(),
                end: Utc::now().into(),
                cameras: Default::default(),
            }),
        };

        let task2 = ArchiveTask {
            birth: Utc::now(),
            api_url: "http://localhost".parse().unwrap(),
            op: ArchiveOperation::Event(Event {
                metadata: EventMetadata {
                    id: "test".to_owned(),
                    timestamp: ts,
                },
                reasons: vec![EventReason {
                    timestamp: Utc::now().into(),
                    reason: "bbb".to_owned(),
                }],
                start: Utc::now().into(),
                end: Utc::now().into(),
                cameras: Default::default(),
            }),
        };

        queue.push(task1);
        queue.push(other_task.clone());
        queue.push(task2.clone());

        assert_eq!(queue.queue.len(), 2);
        assert_eq!(queue.queue[0], other_task);
        assert_eq!(queue.queue[1], task2);
    }

    #[tokio::test]
    async fn newer_event_removes_older_instances_out_of_order() {
        let store = Arc::new(InMemory::new());
        let task_ttl = Duration::hours(1);

        let mut queue = ArchiveRetryQueue::new(store, task_ttl).await;

        let ts = (Utc::now() - Duration::seconds(10)).into();

        let task1 = ArchiveTask {
            birth: Utc::now() - Duration::seconds(5),
            api_url: "http://localhost".parse().unwrap(),
            op: ArchiveOperation::Event(Event {
                metadata: EventMetadata {
                    id: "test".to_owned(),
                    timestamp: ts,
                },
                reasons: vec![EventReason {
                    timestamp: Utc::now().into(),
                    reason: "aaa".to_owned(),
                }],
                start: Utc::now().into(),
                end: Utc::now().into(),
                cameras: Default::default(),
            }),
        };

        let other_task = ArchiveTask {
            birth: Utc::now() - Duration::seconds(5),
            api_url: "http://localhost".parse().unwrap(),
            op: ArchiveOperation::Event(Event {
                metadata: EventMetadata {
                    id: "not a test".to_owned(),
                    timestamp: ts,
                },
                reasons: Default::default(),
                start: Utc::now().into(),
                end: Utc::now().into(),
                cameras: Default::default(),
            }),
        };

        let task2 = ArchiveTask {
            birth: Utc::now(),
            api_url: "http://localhost".parse().unwrap(),
            op: ArchiveOperation::Event(Event {
                metadata: EventMetadata {
                    id: "test".to_owned(),
                    timestamp: ts,
                },
                reasons: vec![EventReason {
                    timestamp: Utc::now().into(),
                    reason: "bbb".to_owned(),
                }],
                start: Utc::now().into(),
                end: Utc::now().into(),
                cameras: Default::default(),
            }),
        };

        queue.push(task2.clone());
        queue.push(other_task.clone());
        queue.push(task1);

        assert_eq!(queue.queue.len(), 2);
        assert_eq!(queue.queue[0], task2);
        assert_eq!(queue.queue[1], other_task);
    }
}
