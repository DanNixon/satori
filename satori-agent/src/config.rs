use serde::Deserialize;
use serde_with::{DurationSeconds, serde_as};
use std::{path::PathBuf, time::Duration};
use url::Url;

#[serde_as]
#[derive(Clone, Deserialize)]
pub(crate) struct Config {
    pub(crate) video_directory: PathBuf,

    pub(crate) stream: StreamConfig,

    #[serde_as(as = "DurationSeconds<u64>")]
    pub(crate) ffmpeg_restart_delay: Duration,
}

impl Config {
    pub(crate) fn get_disk_usage(&self) -> std::io::Result<u64> {
        crate::utils::get_size(&self.video_directory)
    }
}

#[derive(Clone, Deserialize)]
pub(crate) struct StreamConfig {
    pub(crate) url: Url,

    pub(crate) ffmpeg_input_args: Vec<String>,

    pub(crate) hls_segment_time: i32,
    pub(crate) hls_retained_segment_count: i32,
}
