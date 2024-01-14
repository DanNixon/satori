use satori_common::mqtt::MqttConfig;
use satori_storage::StorageConfig;
use serde::Deserialize;
use serde_with::{serde_as, DurationMilliSeconds};
use std::{path::PathBuf, time::Duration};

#[serde_as]
#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) queue_file: PathBuf,

    #[serde_as(as = "DurationMilliSeconds<u64>")]
    pub(crate) interval: Duration,

    pub(crate) mqtt: MqttConfig,

    pub(crate) storage: StorageConfig,
}
