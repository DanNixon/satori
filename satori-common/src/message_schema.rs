use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_with::{DurationSeconds, serde_as};
use std::{path::PathBuf, time::Duration};
use url::Url;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum Message {
    TriggerCommand(TriggerCommand),
    ArchiveCommand(ArchiveCommand),
}

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
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ArchiveCommand {
    EventMetadata(crate::event::Event),
    Segments(ArchiveSegmentsCommand),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArchiveSegmentsCommand {
    pub camera_name: String,
    pub camera_url: Url,
    pub segment_list: Vec<PathBuf>,
}
