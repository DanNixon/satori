use chrono::{DateTime, FixedOffset, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tracing::error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Event {
    pub metadata: EventMetadata,

    pub reasons: Vec<EventReason>,

    pub start: DateTime<FixedOffset>,
    pub end: DateTime<FixedOffset>,

    pub cameras: Vec<CameraSegments>,
}

impl Event {
    pub fn should_expire(&self, ttl: Duration) -> bool {
        self.end + chrono::Duration::from_std(ttl).unwrap() < Utc::now()
    }
}

impl From<crate::Trigger> for Event {
    fn from(trigger: crate::Trigger) -> Self {
        Self {
            start: trigger.start_time(),
            end: trigger.end_time(),
            reasons: vec![EventReason {
                timestamp: trigger.metadata.timestamp,
                reason: trigger.reason,
            }],
            metadata: trigger.metadata,
            cameras: trigger
                .cameras
                .into_iter()
                .map(|c| CameraSegments {
                    name: c,
                    segment_list: vec![],
                })
                .collect(),
        }
    }
}

/// Common event metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventMetadata {
    /// String used to uniquely identify different distinct trigger scenarios.
    pub id: String,

    /// Time that the trgger was triggered.
    pub timestamp: DateTime<FixedOffset>,
}

impl EventMetadata {
    pub fn get_filename(&self) -> PathBuf {
        format!(
            "{}_{}.json",
            self.timestamp.to_rfc3339_opts(SecondsFormat::Secs, false),
            self.id,
        )
        .into()
    }

    #[tracing::instrument]
    pub fn from_filename(path: &Path) -> Result<Self, ()> {
        let path = path.display().to_string();

        let re =
            regex::Regex::new(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2})_(.+).json")
                .unwrap();

        let captures = re.captures(&path).ok_or_else(|| {
            error!("Regex match failed on filename");
        })?;

        let id = captures.get(2).unwrap().as_str().into();

        let timestamp = captures.get(1).unwrap().as_str();
        let timestamp = DateTime::parse_from_rfc3339(timestamp).map_err(|err| {
            error!("Failed to parse timestamp, reason: {}", err);
        })?;

        Ok(Self { id, timestamp })
    }
}

/// A timestamped reason for an event being recorded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EventReason {
    /// Timestamp when this reason occurred
    pub timestamp: DateTime<FixedOffset>,

    /// String description of the reason
    pub reason: String,
}

/// A collection of video segments for a specific camera.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CameraSegments {
    /// Name of the camera
    pub name: String,

    /// List of segments used from this camera
    pub segment_list: Vec<PathBuf>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_reasons() {
        let t = crate::Trigger {
            metadata: EventMetadata {
                id: "trig1".into(),
                timestamp: (Utc::now() - chrono::Duration::minutes(13)).into(),
            },
            reason: "Something happened".into(),
            cameras: vec!["camera-1".into()],
            pre: Duration::from_secs(90),
            post: Duration::from_secs(120),
        };
        let expected_timestamp = t.metadata.timestamp;
        let e: Event = t.into();

        assert_eq!(
            e.reasons,
            vec![EventReason {
                timestamp: expected_timestamp,
                reason: "Something happened".to_string()
            }]
        );
    }

    #[test]
    fn test_should_expire() {
        let t = crate::Trigger {
            metadata: EventMetadata {
                id: "trig1".into(),
                timestamp: (Utc::now() - chrono::Duration::minutes(13)).into(),
            },
            reason: "Something happened".into(),
            cameras: vec!["camera-1".into()],
            pre: Duration::from_secs(90),
            post: Duration::from_secs(120),
        };
        let e: Event = t.into();

        assert!(e.should_expire(Duration::from_secs(600)));
    }

    #[test]
    fn test_should_not_expire() {
        let t = crate::Trigger {
            metadata: EventMetadata {
                id: "trig1".into(),
                timestamp: Utc::now().into(),
            },
            reason: "Something happened".into(),
            cameras: vec!["camera-1".into()],
            pre: Duration::from_secs(90),
            post: Duration::from_secs(150),
        };
        let e: Event = t.into();

        assert!(!e.should_expire(Duration::from_secs(600)));
    }
}
