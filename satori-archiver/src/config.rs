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

#[derive(Debug, Deserialize)]
pub(crate) struct KafkaConfig {
    pub(crate) brokers: String,
    pub(crate) consumer_group: String,

    pub(crate) archive_command_topic: String,
}
