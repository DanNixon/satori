mod streamer;
pub(crate) use self::streamer::Streamer;

mod version;
pub(crate) use self::version::get_ffmpeg_version;

use serde::Deserialize;
use url::Url;

#[derive(Clone, Deserialize)]
pub(crate) struct StreamerConfig {
    pub(crate) url: Url,

    pub(crate) ffmpeg_input_args: Vec<String>,

    pub(crate) hls_segment_time: i32,
    pub(crate) hls_retained_segment_count: i32,
}
