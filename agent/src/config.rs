use crate::ffmpeg::StreamerConfig;
use byte_unit::Byte;
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Deserialize)]
pub(crate) struct Config {
    pub(crate) video_directory: PathBuf,

    pub(crate) stream: StreamerConfig,
}

impl Config {
    pub(crate) fn get_disk_usage(&self) -> std::io::Result<Byte> {
        get_size(&self.video_directory)
    }
}

pub(crate) fn get_size<P>(path: P) -> std::io::Result<Byte>
where
    P: AsRef<Path>,
{
    let mut result: u128 = 0;

    for entry in fs::read_dir(&path)? {
        let path = entry?.path();

        if path.is_file() {
            result += path.metadata()?.len() as u128;
        } else {
            result += get_size(path)?.get_bytes();
        }
    }

    Ok(Byte::from_bytes(result))
}
