use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct CamerasConfig {
    cameras: Vec<CameraConfig>,
}

impl CamerasConfig {
    pub fn into_map(self) -> HashMap<String, Url> {
        let mut ret = HashMap::new();
        for c in self.cameras {
            ret.insert(c.name, c.url);
        }
        ret
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CameraConfig {
    name: String,
    url: Url,
}
