use satori_common::kafka::KafkaConfig;
use satori_storage::StorageConfig;
use serde::Deserialize;
use serde_with::{DurationMilliSeconds, serde_as};
use std::{path::PathBuf, time::Duration};

#[serde_as]
#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) queue_file: PathBuf,

    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub(crate) interval: Duration,

    pub(crate) kafka: KafkaConfig,

    pub(crate) storage: StorageConfig,
}
