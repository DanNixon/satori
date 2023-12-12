use super::{CliExecute, CliResult};
use async_trait::async_trait;
use clap::Parser;
use satori_common::{
    mqtt::{AsyncClientExt, MqttClient, MqttConfig},
    Message,
};
use std::{path::PathBuf, time::Duration};

/// Manually send a trigger command.
#[derive(Debug, Clone, Parser)]
pub(crate) struct TriggerCommand {
    /// Path to MQTT configuration.
    #[arg(long)]
    mqtt: PathBuf,

    /// String used to uniquely identify different distinct trigger scenarios.
    #[arg(long)]
    id: String,

    /// Timestamp of this trigger, defaults to now if not specified.
    #[arg(long)]
    timestamp: Option<chrono::DateTime<chrono::FixedOffset>>,

    /// Name of the cameras that are affected by the trigger.
    #[arg(long)]
    camera: Option<Vec<String>>,

    /// A human readable reason for this trigger.
    #[arg(long)]
    reason: Option<String>,

    /// Time into the past.
    #[arg(long)]
    pre: Option<u64>,

    /// Time into the future.
    #[arg(long)]
    post: Option<u64>,
}

#[async_trait]
impl CliExecute for TriggerCommand {
    async fn execute(&self) -> CliResult {
        let mqtt_config: MqttConfig = satori_common::load_config_file(&self.mqtt);
        let mut mqtt_client: MqttClient = mqtt_config.into();

        let trigger = satori_common::TriggerCommand {
            id: self.id.clone(),
            timestamp: self.timestamp,
            cameras: self.camera.clone(),
            reason: self.reason.clone(),
            pre: self.pre.map(Duration::from_secs),
            post: self.post.map(Duration::from_secs),
        };
        let message = Message::TriggerCommand(trigger);

        let mut client = mqtt_client.client();
        let topic = mqtt_client.topic();
        client.publish_json(topic, &message).await;
        mqtt_client.poll_until_message_is_sent().await;

        mqtt_client.disconnect().await;

        Ok(())
    }
}
