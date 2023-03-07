use super::{CliExecute, CliResult};
use async_trait::async_trait;
use clap::{Parser, Subcommand};
use satori_common::{
    mqtt::MqttConfig, ArchiveCommand, CameraSegments, Event, EventMetadata, Message, Trigger,
};
use std::{path::PathBuf, time::Duration};
use tracing::{info, warn};

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
    async fn execute(&self) -> CliResult {
        let mqtt_config: MqttConfig = satori_common::load_config_file(&self.mqtt);
        let mqtt = mqtt_config.build_client(true).await;

        let result = match &self.command {
            DebugSubcommand::DumpMessages => {
                let mut rx = mqtt.rx_channel();
                while let Ok(mqtt_channel_client::Event::Rx(msg)) = rx.recv().await {
                    match serde_json::from_slice::<Message>(msg.payload()) {
                        Ok(msg) => {
                            info!("{:#?}", msg);
                        }
                        Err(_) => {
                            warn!("Message of unknown type: \"{}\"", msg.payload_str());
                        }
                    }
                }
                Ok(())
            }
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
                satori_common::mqtt::send_json(&mqtt, mqtt_config.topic(), &message).map_err(|_| ())
            }
            DebugSubcommand::ArchiveSegments(cmd) => {
                let segments = CameraSegments {
                    name: cmd.camera.clone(),
                    segment_list: cmd.filename.clone(),
                };
                let message = Message::ArchiveCommand(ArchiveCommand::Segments(segments));
                satori_common::mqtt::send_json(&mqtt, mqtt_config.topic(), &message).map_err(|_| ())
            }
        };

        tokio::time::sleep(Duration::from_millis(10)).await;
        let _ = mqtt.stop().await;
        result
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

    /// Filenames of segments to archive.
    filename: Vec<PathBuf>,
}
