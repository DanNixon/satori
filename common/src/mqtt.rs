use rumqttc::{AsyncClient, Event, EventLoop, Incoming, MqttOptions, Outgoing, Publish, QoS};
use serde::Deserialize;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
    broker: String,
    port: u16,

    client_id: String,

    username: String,
    password: String,

    topic: String,
}

pub struct MqttClient {
    client: AsyncClient,
    event_loop: EventLoop,

    topic: String,
}

impl From<MqttConfig> for MqttClient {
    fn from(config: MqttConfig) -> Self {
        let mut options = MqttOptions::new(config.client_id, config.broker, config.port);
        options.set_keep_alive(Duration::from_secs(5));
        options.set_credentials(config.username, config.password);

        let (client, event_loop) = AsyncClient::new(options, 64);

        Self {
            client,
            event_loop,
            topic: config.topic,
        }
    }
}

impl MqttClient {
    pub fn client(&self) -> AsyncClient {
        self.client.clone()
    }

    pub async fn poll(&mut self) -> Option<Publish> {
        match self.event_loop.poll().await {
            Ok(Event::Incoming(Incoming::Publish(event))) => {
                if event.topic == self.topic {
                    Some(event)
                } else {
                    None
                }
            }
            Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                info!("Connected");
                self.subscribe_to_topic().await;
                None
            }
            Ok(Event::Incoming(Incoming::Disconnect)) => {
                warn!("Disconnected");
                None
            }
            Ok(_) => None,
            Err(e) => {
                warn!("rumqttc error: {:?}", e);
                None
            }
        }
    }

    pub async fn disconnect(&mut self) {
        self.client.disconnect().await.unwrap();

        loop {
            match self.event_loop.poll().await {
                Ok(Event::Outgoing(Outgoing::Disconnect)) => {
                    info!("Disconnected successfully");
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("rumqttc error: {:?}", e);
                }
            }
        }
    }

    pub async fn poll_until_message_is_sent(&mut self) {
        let mut packet_id = None;

        loop {
            match self.event_loop.poll().await {
                Ok(Event::Outgoing(Outgoing::Publish(id))) => {
                    info!("Outgoing publish with packet ID {id}");
                    packet_id = Some(id);
                }
                Ok(Event::Incoming(Incoming::PubAck(event))) => {
                    if packet_id == Some(event.pkid) {
                        info!(
                            "Incomming puback matches packet ID, QoS 1 message sent successfully"
                        );
                        break;
                    }
                }
                Ok(Event::Incoming(Incoming::PubComp(event))) => {
                    if packet_id == Some(event.pkid) {
                        info!(
                            "Incomming pubcomp matches packet ID, QoS 2 message sent successfully"
                        );
                        break;
                    }
                }
                Ok(_) => {}
                Err(e) => {
                    warn!("rumqttc error: {:?}", e);
                }
            }
        }
    }

    async fn subscribe_to_topic(&mut self) {
        if let Err(e) = self
            .client
            .subscribe(self.topic.clone(), QoS::ExactlyOnce)
            .await
        {
            error!("Failed to subscribe to topic: {:?}", e);
        }
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }
}

#[async_trait::async_trait]
pub trait AsyncClientExt {
    async fn publish_json<T: serde::Serialize + Sync>(&mut self, topic: &str, payload: &T);
}

#[async_trait::async_trait]
impl AsyncClientExt for AsyncClient {
    async fn publish_json<T: serde::Serialize + Sync>(&mut self, topic: &str, payload: &T) {
        let payload = serde_json::to_vec(payload).expect("Message should be serialized to JSON");

        if let Err(e) = self.publish(topic, QoS::ExactlyOnce, false, payload).await {
            error!("Failed to publish message: {:?}", e);
        }
    }
}

pub trait PublishExt {
    fn try_payload_str(&self) -> Result<&str, std::str::Utf8Error>;
    fn try_payload_from_json<'a, T: serde::Deserialize<'a>>(&'a self) -> serde_json::Result<T>;
}

impl PublishExt for Publish {
    fn try_payload_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(&self.payload)
    }

    fn try_payload_from_json<'a, T: serde::Deserialize<'a>>(&'a self) -> serde_json::Result<T> {
        serde_json::from_slice(&self.payload)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use satori_testing_utils::{MosquittoDriver, TestMqttClient};

    #[ctor::ctor]
    fn init() {
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

    #[tokio::test]
    async fn client_poll_until_message_is_sent_qos_atleastonce() {
        let topic = "test";
        let payload = "Hello, world!".as_bytes();

        let mosquitto = MosquittoDriver::default();
        let mut test_client = TestMqttClient::new(mosquitto.port()).await;
        test_client
            .client()
            .subscribe(topic, QoS::AtLeastOnce)
            .await
            .unwrap();

        let config = MqttConfig {
            broker: "localhost".to_string(),
            port: mosquitto.port(),
            client_id: "a-unit-test".to_string(),
            username: "".to_string(),
            password: "".to_string(),
            topic: topic.to_string(),
        };

        let mut client: MqttClient = config.into();

        client
            .client()
            .publish(topic, QoS::AtLeastOnce, false, payload)
            .await
            .unwrap();

        client.poll_until_message_is_sent().await;

        let msg = test_client
            .wait_for_message(Duration::from_secs(5))
            .await
            .expect("a message should have been received");
        assert_eq!(msg.topic, topic);
        assert_eq!(msg.payload, "Hello, world!");

        client.disconnect().await;
    }

    #[tokio::test]
    async fn client_poll_until_message_is_sent_qos_exactlyonce() {
        let topic = "test";
        let payload = "Hello, world!".as_bytes();

        let mosquitto = MosquittoDriver::default();
        let mut test_client = TestMqttClient::new(mosquitto.port()).await;
        test_client
            .client()
            .subscribe(topic, QoS::ExactlyOnce)
            .await
            .unwrap();

        let config = MqttConfig {
            broker: "localhost".to_string(),
            port: mosquitto.port(),
            client_id: "a-unit-test".to_string(),
            username: "".to_string(),
            password: "".to_string(),
            topic: topic.to_string(),
        };

        let mut client: MqttClient = config.into();

        client
            .client()
            .publish(topic, QoS::ExactlyOnce, false, payload)
            .await
            .unwrap();

        client.poll_until_message_is_sent().await;

        let msg = test_client
            .wait_for_message(Duration::from_secs(5))
            .await
            .expect("a message should have been received");
        assert_eq!(msg.topic, topic);
        assert_eq!(msg.payload, "Hello, world!");

        client.disconnect().await;
    }
}
