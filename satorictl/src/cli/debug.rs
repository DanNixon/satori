use super::CliExecute;
use async_trait::async_trait;
use clap::{Parser, Subcommand};
use satori_common::{
    ArchiveCommand, ArchiveSegmentsCommand, Event, EventMetadata, Message, Trigger,
    mqtt::{AsyncClientExt, MqttClient, MqttConfig, PublishExt},
};
use std::{path::PathBuf, time::Duration};
use tracing::{info, warn};
use url::Url;

/// Debugging operations.
#[derive(Debug, Clone, Parser)]
pub(crate) struct DebugCommand {
    /// Path to MQTT configuration
    #[arg(long)]
    mqtt: PathBuf,

    #[command(subcommand)]
    command: DebugSubcommand,
}

#[async_trait]
impl CliExecute for DebugCommand {
    async fn execute(&self) -> miette::Result<()> {
        let mqtt_config: MqttConfig = satori_common::load_config_file(&self.mqtt);
        let mut mqtt_client: MqttClient = mqtt_config.into();

        match &self.command {
            DebugSubcommand::DumpMessages => loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        info!("Exiting.");
                        break;
                    }
                    msg = mqtt_client.poll() => {
                        if let Some(msg) = msg {
                            match msg.try_payload_from_json::<satori_common::Message>() {
                                Ok(msg) => {
                                    info!("{:#?}", msg);
                                }
                                Err(_) => {
                                    warn!("Failed to parse message");
                                }
                            }
                        }
                    }
                }
            },
            DebugSubcommand::ArchiveEvent(cmd) => {
                let trigger = Trigger {
                    metadata: EventMetadata {
                        id: "default-trigger".into(),
                        timestamp: chrono::Utc::now().into(),
                    },
                    reason: cmd.reason.clone(),
                    cameras: vec!["camera-1".into(), "camera-2".into()],
                    pre: Duration::from_secs(120),
                    post: Duration::from_secs(60),
                };
                let mut event: Event = trigger.into();
                event.cameras[0].segment_list =
                    vec!["one.ts".into(), "two.ts".into(), "three.ts".into()];
                event.cameras[1].segment_list =
                    vec!["four.ts".into(), "five.ts".into(), "six.ts".into()];
                let message = Message::ArchiveCommand(ArchiveCommand::EventMetadata(event));

                let mut client = mqtt_client.client();
                let topic = mqtt_client.topic();
                client.publish_json(topic, &message).await;
                mqtt_client.poll_until_message_is_sent().await;
            }
            DebugSubcommand::ArchiveSegments(cmd) => {
                let message =
                    Message::ArchiveCommand(ArchiveCommand::Segments(ArchiveSegmentsCommand {
                        camera_name: cmd.camera.clone(),
                        camera_url: cmd.url.clone(),
                        segment_list: cmd.filename.clone(),
                    }));

                let mut client = mqtt_client.client();
                let topic = mqtt_client.topic();
                client.publish_json(topic, &message).await;
                mqtt_client.poll_until_message_is_sent().await;
            }
        }

        mqtt_client.disconnect().await;

        Ok(())
    }
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum DebugSubcommand {
    DumpMessages,
    ArchiveEvent(DebugArchiveEventCommand),
    ArchiveSegments(DebugArchiveSegmentsCommand),
}

/// Send a dummy event to listening archivers.
#[derive(Debug, Clone, Parser)]
pub(crate) struct DebugArchiveEventCommand {
    /// Reason to include in dummy event.
    #[arg(long, default_value = "Debug event from satorictl")]
    reason: String,
}

/// Send a video segment list to listening archivers.
#[derive(Debug, Clone, Parser)]
pub(crate) struct DebugArchiveSegmentsCommand {
    /// Name of the camera to retrieve segments from.
    #[arg(long)]
    camera: String,

    /// URL of the camera's HLS stream.
    #[arg(long)]
    url: Url,

    /// Filenames of segments to archive.
    filename: Vec<PathBuf>,
}
