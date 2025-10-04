use crate::{Provider, StorageError, StorageProvider, StorageResult};
use chrono::{DateTime, FixedOffset};
use satori_common::EventMetadata;
use std::path::PathBuf;
use tracing::{error, info};

pub async fn prune_events_older_than(
    storage: Provider,
    time: DateTime<FixedOffset>,
) -> StorageResult<()> {
    info!("Getting event list");
    let event_filenames = storage.list_events().await?;

    let mut result = Ok(());

    // Filter the list of event filenames, removing those that we want to keep
    let event_files_to_delete: Vec<PathBuf> = event_filenames
        .into_iter()
        .filter(|event| match EventMetadata::from_filename(event) {
            Ok(metadata) => metadata.timestamp < time,
            Err(_) => {
                error!("Failed to parse metadata from filename");
                result = Err(StorageError::WorkflowPartialError);
                false
            }
        })
        .collect();

    // Delete all the events marked for deletion
    for filename in event_files_to_delete {
        info!("Pruning event: {}", filename.display());
        if let Err(err) = storage.delete_event_filename(&filename).await {
            error!(
                "Failed to remove event file {}, reason: {}",
                filename.display(),
                err
            );
            result = Err(StorageError::WorkflowPartialError);
        }
    }

    result
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::providers::dummy::DummyConfig;
    use chrono::{FixedOffset, NaiveDate, Utc};
    use satori_common::{Event, EventMetadata};

    async fn build_test_storage() -> Provider {
        let provider = crate::StorageConfig::Dummy(DummyConfig::default()).create_provider();

        provider
            .put_event(&Event {
                metadata: EventMetadata {
                    id: "test-1".into(),
                    timestamp: NaiveDate::from_ymd_opt(2023, 3, 1)
                        .unwrap()
                        .and_hms_opt(12, 0, 0)
                        .unwrap()
                        .and_local_timezone(FixedOffset::east_opt(0).unwrap())
                        .unwrap(),
                },
                start: Utc::now().into(),
                end: Utc::now().into(),
                reasons: Default::default(),
                cameras: Default::default(),
            })
            .await
            .unwrap();

        provider
            .put_event(&Event {
                metadata: EventMetadata {
                    id: "test-2".into(),
                    timestamp: NaiveDate::from_ymd_opt(2023, 3, 1)
                        .unwrap()
                        .and_hms_opt(12, 10, 0)
                        .unwrap()
                        .and_local_timezone(FixedOffset::east_opt(0).unwrap())
                        .unwrap(),
                },
                start: Utc::now().into(),
                end: Utc::now().into(),
                reasons: Default::default(),
                cameras: Default::default(),
            })
            .await
            .unwrap();

        provider
            .put_event(&Event {
                metadata: EventMetadata {
                    id: "test-3".into(),
                    timestamp: NaiveDate::from_ymd_opt(2023, 3, 2)
                        .unwrap()
                        .and_hms_opt(7, 0, 0)
                        .unwrap()
                        .and_local_timezone(FixedOffset::east_opt(0).unwrap())
                        .unwrap(),
                },
                start: Utc::now().into(),
                end: Utc::now().into(),
                reasons: Default::default(),
                cameras: Default::default(),
            })
            .await
            .unwrap();

        provider
    }

    #[tokio::test]
    async fn test_prune_events_older_than_noop() {
        let provider = build_test_storage().await;

        prune_events_older_than(
            provider.clone(),
            NaiveDate::from_ymd_opt(2023, 3, 1)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap()
                .and_local_timezone(FixedOffset::east_opt(0).unwrap())
                .unwrap(),
        )
        .await
        .unwrap();

        let events = provider.list_events().await.unwrap();
        assert_eq!(events.len(), 3);
    }

    #[tokio::test]
    async fn test_prune_events_older_than() {
        let provider = build_test_storage().await;

        prune_events_older_than(
            provider.clone(),
            NaiveDate::from_ymd_opt(2023, 3, 1)
                .unwrap()
                .and_hms_opt(21, 0, 0)
                .unwrap()
                .and_local_timezone(FixedOffset::east_opt(0).unwrap())
                .unwrap(),
        )
        .await
        .unwrap();

        let events = provider.list_events().await.unwrap();
        assert_eq!(events.len(), 1);
    }
}
