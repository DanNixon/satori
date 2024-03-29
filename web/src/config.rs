use serde::Deserialize;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) cameras: CameraDetails,
}

pub(crate) type CameraDetails = HashMap<String, CameraDetail>;

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct CameraDetail {
    thumbnail_url: Url,
    mjpeg_url: Url,
    hls_url: Url,
}
