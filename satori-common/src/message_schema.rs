use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_with::{DurationSeconds, serde_as};
use std::time::Duration;
use url::Url;

#[serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TriggerCommand {
    pub id: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<DateTime<FixedOffset>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub cameras: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,

    #[serde_as(as = "Option<DurationSeconds<u64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pre: Option<Duration>,

    #[serde_as(as = "Option<DurationSeconds<u64>>")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveSegmentCommand {
    pub segment_url: Url,
}
