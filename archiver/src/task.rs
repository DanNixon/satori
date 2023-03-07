use crate::{
    error::{ArchiverError, ArchiverResult},
    Context,
};
use bytes::Bytes;
use satori_common::Event;
use satori_storage::StorageProvider;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, info};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ArchiveTask {
    EventMetadata(Event),
    CameraSegment(CameraSegment),
}

impl ArchiveTask {
    #[tracing::instrument(skip_all)]
    pub(crate) async fn run(&self, context: &Context) -> ArchiverResult<()> {
        match &self {
            Self::EventMetadata(event) => self.run_event(context, event).await,
            Self::CameraSegment(segment) => self.run_segment(context, segment).await,
        }
    }

    #[tracing::instrument(skip(context))]
    async fn run_event(&self, context: &Context, event: &Event) -> ArchiverResult<()> {
        info!("Saving event");
        Ok(context.storage.put_event(event).await?)
    }

    #[tracing::instrument(skip(context))]
    async fn run_segment(&self, context: &Context, segment: &CameraSegment) -> ArchiverResult<()> {
        info!("Saving segment");
        let data = segment.get(context).await?;
        Ok(context
            .storage
            .put_segment(&segment.camera_name, &segment.filename, data)
            .await?)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CameraSegment {
    pub(crate) camera_name: String,
    pub(crate) filename: PathBuf,
}

impl CameraSegment {
    #[tracing::instrument(skip_all)]
    pub(crate) async fn get(&self, context: &Context) -> ArchiverResult<Bytes> {
        let url = context
            .cameras
            .get_url(&self.camera_name)
            .ok_or(ArchiverError::CameraNotFound)?;
        let url = get_segment_url(url, &self.filename)?;
        debug!("Segment URL: {url}");

        let req = context.http_client.get(url).send().await?;
        Ok(req.bytes().await?)
    }
}

fn get_segment_url(hls_url: Url, segment_filename: &Path) -> ArchiverResult<Url> {
    let mut url = hls_url;
    url.path_segments_mut()
        .map_err(|_| ArchiverError::UrlError)?
        .pop()
        .push(segment_filename.to_str().ok_or(ArchiverError::UrlError)?);
    Ok(url)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_segment_url_1() {
        let hls_url = Url::parse("http://localhost:8080/camera/stream.m3u8").unwrap();
        let segment_filename: PathBuf = "a_file.ts".into();
        assert_eq!(
            get_segment_url(hls_url, &segment_filename).unwrap(),
            Url::parse("http://localhost:8080/camera/a_file.ts").unwrap()
        )
    }

    #[test]
    fn test_get_segment_url_2() {
        let hls_url = Url::parse("http://localhost:8080/stream.m3u8").unwrap();
        let segment_filename: PathBuf = "a_file.ts".into();
        assert_eq!(
            get_segment_url(hls_url, &segment_filename).unwrap(),
            Url::parse("http://localhost:8080/a_file.ts").unwrap()
        )
    }
}
