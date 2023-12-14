use serde::Deserialize;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    cameras: HashMap<String, CameraDetails>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CameraDetails {
    hls_stream: Url,
    agent_metrics: Url,
}
