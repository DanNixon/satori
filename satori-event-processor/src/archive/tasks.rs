use chrono::{DateTime, Utc};
use miette::{Context, IntoDiagnostic};
use satori_common::{ArchiveSegmentCommand, Event};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ArchiveTask {
    pub(crate) birth: DateTime<Utc>,
    pub(crate) api_url: Url,
    pub(super) op: ArchiveOperation,
}

impl ArchiveTask {
    fn new(api_urls: &[Url], op: ArchiveOperation) -> Vec<Self> {
        let birth = Utc::now();

        api_urls
            .iter()
            .map(|api_url| Self {
                birth,
                api_url: api_url.to_owned(),
                op: op.clone(),
            })
            .collect()
    }

    pub(crate) fn new_event(api_urls: &[Url], event: Event) -> Vec<Self> {
        Self::new(api_urls, ArchiveOperation::Event(event))
    }

    pub(crate) fn new_segment(api_urls: &[Url], camera_name: String, segment: Url) -> Vec<Self> {
        Self::new(
            api_urls,
            ArchiveOperation::Segment {
                camera_name,
                url: segment,
            },
        )
    }

    #[tracing::instrument(skip(http_client))]
    pub(crate) async fn execute(&self, http_client: &reqwest::Client) -> miette::Result<()> {
        let response = match &self.op {
            ArchiveOperation::Event(event) => {
                let url = self.api_url.join("event").into_diagnostic()?;

                http_client
                    .post(url)
                    .json(&event)
                    .send()
                    .await
                    .into_diagnostic()
                    .wrap_err("Storage API call failed")?
            }
            ArchiveOperation::Segment { camera_name, url } => {
                let api_url = self
                    .api_url
                    .join(&format!("video/{camera_name}"))
                    .into_diagnostic()?;

                let cmd = ArchiveSegmentCommand {
                    segment_url: url.to_owned(),
                };

                http_client
                    .post(api_url)
                    .json(&cmd)
                    .send()
                    .await
                    .into_diagnostic()
                    .wrap_err("Storage API call failed")?
            }
        };

        let _ = response
            .error_for_status_ref()
            .into_diagnostic()
            .wrap_err("Storage API returned error status")?;

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ArchiveOperation {
    Event(Event),
    Segment { camera_name: String, url: Url },
}
