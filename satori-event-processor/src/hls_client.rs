use crate::error::{EventProcessorError, EventProcessorResult};
use satori_common::camera_config::CamerasConfig;
use std::collections::HashMap;
use tracing::{debug, error};
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
    pub(crate) fn get_camera_url(&self, camera: &str) -> EventProcessorResult<Url> {
        self.camera_urls
            .get(camera)
            .ok_or_else(|| EventProcessorError::NoSuchCamera(camera.into()))
            .cloned()
    }

    #[tracing::instrument(skip(self))]
    pub(crate) async fn get_playlist(
        &self,
        camera: &str,
    ) -> EventProcessorResult<m3u8_rs::MediaPlaylist> {
        let url = self.get_camera_url(camera)?;
        let body = self.http_client.get(url).send().await?.bytes().await?;
        parse_playlist(body)
    }
}

#[tracing::instrument(skip_all)]
fn parse_playlist(data: bytes::Bytes) -> EventProcessorResult<m3u8_rs::MediaPlaylist> {
    match m3u8_rs::parse_playlist_res(&data) {
        Ok(pl) => {
            if let m3u8_rs::Playlist::MediaPlaylist(pl) = pl {
                debug!("Playlist length: {}", pl.segments.len());
                Ok(pl)
            } else {
                error!("Did not find a media playlist");
                Err(EventProcessorError::PlaylistParseError)
            }
        }
        Err(err) => {
            error!("Failed to parse playlist, reason: {}", err);
            Err(EventProcessorError::PlaylistParseError)
        }
    }
}
