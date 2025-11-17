use crate::{EventMetadata, TriggerCommand};
use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use serde_with::{DurationSeconds, serde_as};
use std::time::Duration;

#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Trigger {
    pub metadata: EventMetadata,

    /// A human readable reason for this trigger.
    pub reason: String,

    /// Name of the cameras that are affected by the trigger.
    pub cameras: Vec<String>,

    /// Time into the past.
    #[serde_as(as = "DurationSeconds<u64>")]
    pub pre: Duration,

    /// Time into the future.
    #[serde_as(as = "DurationSeconds<u64>")]
    pub post: Duration,
}

impl Trigger {
    pub fn from_default_and_command(default: &TriggerTemplate, cmd: &TriggerCommand) -> Self {
        Self {
            metadata: EventMetadata {
                id: cmd.id.clone(),
                timestamp: cmd.timestamp.unwrap_or_else(|| Utc::now().into()),
            },
            reason: cmd.reason.clone().unwrap_or_else(|| default.reason.clone()),
            cameras: cmd
                .cameras
                .clone()
                .unwrap_or_else(|| default.cameras.clone()),
            pre: cmd.pre.unwrap_or(default.pre),
            post: cmd.post.unwrap_or(default.post),
        }
    }

    pub fn start_time(&self) -> DateTime<FixedOffset> {
        self.metadata.timestamp - chrono::Duration::from_std(self.pre).unwrap()
    }

    pub fn end_time(&self) -> DateTime<FixedOffset> {
        self.metadata.timestamp + chrono::Duration::from_std(self.post).unwrap()
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Clone)]
pub struct TriggerTemplate {
    pub cameras: Vec<String>,

    pub reason: String,

    #[serde_as(as = "DurationSeconds<u64>")]
    pub pre: Duration,

    #[serde_as(as = "DurationSeconds<u64>")]
    pub post: Duration,
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_get_filename() {
        let metadata = EventMetadata {
            timestamp: Utc
                .with_ymd_and_hms(2022, 11, 20, 5, 28, 30)
                .unwrap()
                .into(),
            id: "thing1".into(),
        };

        let expected = "2022-11-20T05:28:30+00:00_thing1.json";
        assert_eq!(metadata.filename(), expected);
    }

    #[test]
    fn test_from_filename() {
        let filename = "2022-11-20T05:28:30+00:00_thing1.json";

        let expected = EventMetadata {
            timestamp: Utc
                .with_ymd_and_hms(2022, 11, 20, 5, 28, 30)
                .unwrap()
                .into(),
            id: "thing1".into(),
        };
        assert_eq!(EventMetadata::from_filename(filename).unwrap(), expected);
    }

    #[test]
    fn test_from_filename_underscore_in_id() {
        let filename = "2022-11-20T05:28:30+00:00_thing_1.json";

        let expected = EventMetadata {
            timestamp: Utc
                .with_ymd_and_hms(2022, 11, 20, 5, 28, 30)
                .unwrap()
                .into(),
            id: "thing_1".into(),
        };
        assert_eq!(EventMetadata::from_filename(filename).unwrap(), expected);
    }

    #[test]
    fn test_from_filename_fail_no_id() {
        let filename = "2022-11-20T05:28:30+00:00.json";
        assert!(EventMetadata::from_filename(filename).is_err());
    }

    #[test]
    fn test_from_filename_fail_bad_timestamp() {
        let filename = "2022-31-20T05:28:30+00:00_thing1.json";
        assert!(EventMetadata::from_filename(filename).is_err());
    }

    #[test]
    fn test_from_default_and_command_all_default() {
        let default = TriggerTemplate {
            cameras: vec!["camera-1".into(), "camera-2".into()],
            reason: "Something happened".into(),
            pre: Duration::from_secs(60),
            post: Duration::from_secs(120),
        };

        let cmd = TriggerCommand {
            id: "door sensor".into(),
            timestamp: None,
            cameras: None,
            reason: None,
            pre: None,
            post: None,
        };

        let trigger = Trigger::from_default_and_command(&default, &cmd);

        assert_eq!(
            trigger,
            Trigger {
                metadata: EventMetadata {
                    id: "door sensor".into(),
                    timestamp: trigger.metadata.timestamp,
                },
                reason: "Something happened".into(),
                cameras: vec!["camera-1".into(), "camera-2".into()],
                pre: Duration::from_secs(60),
                post: Duration::from_secs(120),
            }
        );

        let trigger_time: DateTime<Utc> = trigger.metadata.timestamp.into();
        assert!(Utc::now() - trigger_time < chrono::Duration::try_milliseconds(100).unwrap());
    }

    #[test]
    fn test_from_default_and_command_all_specified() {
        let default = TriggerTemplate {
            cameras: vec!["camera-1".into(), "camera-2".into()],
            reason: "Something happened".into(),
            pre: Duration::from_secs(60),
            post: Duration::from_secs(120),
        };

        let time = Utc.with_ymd_and_hms(2022, 11, 20, 5, 30, 0).unwrap().into();

        let cmd = TriggerCommand {
            id: "door sensor".into(),
            timestamp: Some(time),
            cameras: Some(vec!["camera-2".into()]),
            reason: Some("Something else happened".into()),
            pre: Some(Duration::from_secs(30)),
            post: Some(Duration::from_secs(60)),
        };

        let trigger = Trigger::from_default_and_command(&default, &cmd);

        assert_eq!(
            trigger,
            Trigger {
                metadata: EventMetadata {
                    id: "door sensor".into(),
                    timestamp: time,
                },
                reason: "Something else happened".into(),
                cameras: vec!["camera-2".into()],
                pre: Duration::from_secs(30),
                post: Duration::from_secs(60),
            }
        );
    }

    #[test]
    fn test_wall_clock_times() {
        let t = Trigger {
            metadata: EventMetadata {
                id: "trig1".into(),
                timestamp: Utc.with_ymd_and_hms(2022, 11, 20, 5, 30, 0).unwrap().into(),
            },
            reason: "Something happened".into(),
            cameras: vec!["camera-1".into()],
            pre: Duration::from_secs(90),
            post: Duration::from_secs(150),
        };

        assert_eq!(
            t.start_time(),
            Utc.with_ymd_and_hms(2022, 11, 20, 5, 28, 30).unwrap(),
        );
        assert_eq!(
            t.end_time(),
            Utc.with_ymd_and_hms(2022, 11, 20, 5, 32, 30).unwrap(),
        );
    }
}
