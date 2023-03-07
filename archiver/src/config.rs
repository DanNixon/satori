use satori_common::{camera_config::CamerasConfig, mqtt::MqttConfig};
use satori_storage::StorageConfig;
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};
use std::{path::PathBuf, time::Duration};

#[serde_as]
#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) queue_file: PathBuf,

    #[serde_as(as = "DurationSeconds<u64>")]
    pub(crate) interval: Duration,

    pub(crate) mqtt: MqttConfig,

    #[serde(flatten)]
    pub(crate) cameras: CamerasConfig,

    pub(crate) storage: StorageConfig,
}
