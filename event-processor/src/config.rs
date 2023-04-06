use satori_common::{
    camera_config::CamerasConfig, mqtt::MqttConfig, Trigger, TriggerCommand, TriggerTemplate,
};
use serde::Deserialize;
use serde_with::{serde_as, DurationSeconds};
use std::{collections::HashMap, path::PathBuf, time::Duration};
use tracing::info;

#[serde_as]
#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) event_file: PathBuf,

    #[serde_as(as = "DurationSeconds<u64>")]
    pub(crate) interval: Duration,

    #[serde_as(as = "DurationSeconds<u64>")]
    pub(crate) event_ttl: Duration,

    pub(crate) mqtt: MqttConfig,

    #[serde(flatten)]
    pub(crate) cameras: CamerasConfig,

    pub(crate) triggers: TriggersConfig,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TriggersConfig {
    /// Trigger configs that are used when a trigger with a specific ID are issued
    pub(crate) templates: HashMap<String, TriggerTemplate>,

    /// Trigger defaults that are used when no matching template is found
    pub(crate) fallback: TriggerTemplate,
}

impl TriggersConfig {
    #[tracing::instrument(skip(self))]
    pub(crate) fn create_trigger(&self, cmd: &TriggerCommand) -> Trigger {
        let template = match self.templates.get(&cmd.id) {
            Some(t) => {
                info!("Found predefined template for ID \"{}\"", cmd.id);
                t
            }
            None => {
                info!("No template matches ID \"{}\", using fallback", cmd.id);
                &self.fallback
            }
        };

        Trigger::from_default_and_command(template, cmd)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use chrono::{TimeZone, Utc};
    use satori_common::EventMetadata;

    #[test]
    fn test_trigger_config_only_fallback() {
        let config = TriggersConfig {
            templates: Default::default(),
            fallback: TriggerTemplate {
                cameras: vec!["camera-1".into(), "camera-2".into(), "camera-3".into()],
                reason: "Something happened".into(),
                pre: Duration::from_secs(60),
                post: Duration::from_secs(120),
            },
        };

        let time = Utc.with_ymd_and_hms(2022, 11, 20, 5, 30, 0).unwrap().into();

        let cmd = TriggerCommand {
            id: "thing".into(),
            timestamp: Some(time),
            cameras: None,
            reason: Some("reason".into()),
            pre: None,
            post: None,
        };

        assert_eq!(
            Trigger {
                metadata: EventMetadata {
                    id: "thing".into(),
                    timestamp: time,
                },
                reason: "reason".into(),
                cameras: vec!["camera-1".into(), "camera-2".into(), "camera-3".into()],
                pre: Duration::from_secs(60),
                post: Duration::from_secs(120),
            },
            config.create_trigger(&cmd)
        );
    }

    #[test]
    fn test_trigger_config_fallback() {
        let config = TriggersConfig {
            templates: HashMap::from([
                (
                    "thing 1".to_string(),
                    TriggerTemplate {
                        cameras: vec!["camera-3".into()],
                        reason: "Something happened - 1".into(),
                        pre: Duration::from_secs(60),
                        post: Duration::from_secs(30),
                    },
                ),
                (
                    "thing 2".to_string(),
                    TriggerTemplate {
                        cameras: vec!["camera-1".into(), "camera-2".into()],
                        reason: "Something happened - 2".into(),
                        pre: Duration::from_secs(60),
                        post: Duration::from_secs(60),
                    },
                ),
            ]),
            fallback: TriggerTemplate {
                cameras: vec!["camera-1".into(), "camera-2".into(), "camera-3".into()],
                reason: "Something happened".into(),
                pre: Duration::from_secs(60),
                post: Duration::from_secs(120),
            },
        };

        let time = Utc.with_ymd_and_hms(2022, 11, 20, 5, 30, 0).unwrap().into();

        let cmd = TriggerCommand {
            id: "thing".into(),
            timestamp: Some(time),
            cameras: None,
            reason: Some("reason".into()),
            pre: None,
            post: None,
        };

        assert_eq!(
            Trigger {
                metadata: EventMetadata {
                    id: "thing".into(),
                    timestamp: time,
                },
                reason: "reason".into(),
                cameras: vec!["camera-1".into(), "camera-2".into(), "camera-3".into()],
                pre: Duration::from_secs(60),
                post: Duration::from_secs(120),
            },
            config.create_trigger(&cmd)
        );
    }

    #[test]
    fn test_trigger_config_template() {
        let config = TriggersConfig {
            templates: HashMap::from([
                (
                    "thing 1".to_string(),
                    TriggerTemplate {
                        cameras: vec!["camera-3".into()],
                        reason: "Something happened - 1".into(),
                        pre: Duration::from_secs(60),
                        post: Duration::from_secs(30),
                    },
                ),
                (
                    "thing 2".to_string(),
                    TriggerTemplate {
                        cameras: vec!["camera-1".into(), "camera-2".into()],
                        reason: "Something happened - 2".into(),
                        pre: Duration::from_secs(60),
                        post: Duration::from_secs(60),
                    },
                ),
            ]),
            fallback: TriggerTemplate {
                cameras: vec!["camera-1".into(), "camera-2".into(), "camera-3".into()],
                reason: "Something happened".into(),
                pre: Duration::from_secs(60),
                post: Duration::from_secs(120),
            },
        };

        let time = Utc.with_ymd_and_hms(2022, 11, 20, 5, 30, 0).unwrap().into();

        let cmd = TriggerCommand {
            id: "thing 1".into(),
            timestamp: Some(time),
            cameras: None,
            reason: Some("reason".into()),
            pre: None,
            post: None,
        };

        assert_eq!(
            Trigger {
                metadata: EventMetadata {
                    id: "thing 1".into(),
                    timestamp: time,
                },
                reason: "reason".into(),
                cameras: vec!["camera-3".into()],
                pre: Duration::from_secs(60),
                post: Duration::from_secs(30),
            },
            config.create_trigger(&cmd)
        );
    }
}
