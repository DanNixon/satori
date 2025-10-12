use m3u8_rs::MediaPlaylist;
use miette::IntoDiagnostic;
use satori_common::camera_config::CamerasConfig;
use std::collections::HashMap;
use url::Url;

pub(crate) struct HlsClient {
    http_client: reqwest::Client,
    camera_urls: HashMap<String, Url>,
}

impl HlsClient {
    pub(crate) fn new(cameras: CamerasConfig) -> Self {
        let http_client = reqwest::ClientBuilder::new()
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap();

        Self {
            http_client,
            camera_urls: cameras.into_map(),
        }
    }

    #[tracing::instrument(skip(self))]
    pub(crate) fn get_camera_url(&self, camera: &str) -> miette::Result<Url> {
        self.camera_urls
            .get(camera)
            .ok_or_else(|| miette::miette!("No such camera: {camera}"))
            .cloned()
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn get_playlist(&self, camera: &str) -> miette::Result<MediaPlaylist> {
        let url = self.get_camera_url(camera)?;

        let body = self
            .http_client
            .get(url)
            .send()
            .await
            .into_diagnostic()?
            .bytes()
            .await
            .into_diagnostic()?;

        satori_common::parse_m3u8_media_playlist(&body)
    }
}
