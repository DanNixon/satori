use serde::Deserialize;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    cameras: HashMap<String, CameraDetails>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CameraDetails {
    thumbnail_url: Url,
    mjpeg_url: Url,
    hls_url: Url,
}
