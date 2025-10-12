use crate::AppContext;
use bytes::Bytes;
use miette::IntoDiagnostic;
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
    pub(crate) async fn run(&self, context: &AppContext) -> miette::Result<()> {
        match &self {
            Self::EventMetadata(event) => self.run_event(context, event).await,
            Self::CameraSegment(segment) => self.run_segment(context, segment).await,
        }
    }

    #[tracing::instrument(skip(context))]
    async fn run_event(&self, context: &AppContext, event: &Event) -> miette::Result<()> {
        info!("Saving event");
        context.storage.put_event(event).await.into_diagnostic()
    }

    #[tracing::instrument(skip(context))]
    async fn run_segment(
        &self,
        context: &AppContext,
        segment: &CameraSegment,
    ) -> miette::Result<()> {
        info!("Saving segment");
        let data = segment.get(context).await?;
        context
            .storage
            .put_segment(&segment.camera_name, &segment.filename, data)
            .await
            .into_diagnostic()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CameraSegment {
    pub(crate) camera_name: String,
    pub(crate) camera_url: Url,
    pub(crate) filename: PathBuf,
}

impl CameraSegment {
    #[tracing::instrument(skip_all)]
    pub(crate) async fn get(&self, context: &AppContext) -> miette::Result<Bytes> {
        let url = get_segment_url(self.camera_url.clone(), &self.filename)?;
        debug!("Segment URL: {url}");

        let req = context
            .http_client
            .get(url)
            .send()
            .await
            .into_diagnostic()?;
        req.bytes().await.into_diagnostic()
    }
}

fn get_segment_url(hls_url: Url, segment_filename: &Path) -> miette::Result<Url> {
    let mut url = hls_url;
    url.path_segments_mut()
        .map_err(|_| miette::miette!("Failed to get URL path segments"))?
        .pop()
        .push(
            segment_filename
                .to_str()
                .ok_or_else(|| miette::miette!("Failed to convert segment filename to string"))?,
        );
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
