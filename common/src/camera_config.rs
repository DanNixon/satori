use serde::Deserialize;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Deserialize)]
pub struct CamerasConfig {
    cameras: Vec<CameraConfig>,
}

impl CamerasConfig {
    pub fn get_url(&self, camera_name: &str) -> Option<Url> {
        self.cameras
            .iter()
            .find(|c| c.name == camera_name)
            .map(|c| c.url.clone())
    }

    pub fn into_map(self) -> HashMap<String, Url> {
        let mut ret = HashMap::new();
        for c in self.cameras {
            ret.insert(c.name, c.url);
        }
        ret
    }
}

#[derive(Debug, Deserialize)]
pub struct CameraConfig {
    name: String,
    url: Url,
}

#[cfg(test)]
mod test {
    use super::*;

    fn get_test_cameras_config() -> CamerasConfig {
        CamerasConfig {
            cameras: vec![
                CameraConfig {
                    name: "camera-1".into(),
                    url: Url::parse("http://camera-1/stream.m3u8").unwrap(),
                },
                CameraConfig {
                    name: "camera-2".into(),
                    url: Url::parse("http://camera-2/stream.m3u8").unwrap(),
                },
                CameraConfig {
                    name: "camera-3".into(),
                    url: Url::parse("http://camera-3/stream.m3u8").unwrap(),
                },
            ],
        }
    }

    #[test]
    fn test_cameras_config_get_url() {
        let config = get_test_cameras_config();
        assert_eq!(
            config.get_url("camera-1"),
            Some(Url::parse("http://camera-1/stream.m3u8").unwrap())
        );
    }

    #[test]
    fn test_cameras_config_get_url_fail() {
        let config = get_test_cameras_config();
        assert_eq!(config.get_url("camera-nope"), None);
    }
}
