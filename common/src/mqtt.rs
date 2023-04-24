use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
pub struct MqttConfig {
    broker: url::Url,

    client_id: String,

    username: String,
    password: String,

    topic: String,
}

impl MqttConfig {
    pub async fn build_client(&self, anon: bool) -> mqtt_channel_client::Client {
        let mut create_options =
            mqtt_channel_client::paho_mqtt::create_options::CreateOptionsBuilder::new()
                .server_uri(self.broker.clone())
                .persistence(mqtt_channel_client::paho_mqtt::PersistenceType::None);
        if !anon {
            create_options = create_options.client_id(&self.client_id);
        }
        let create_options = create_options.finalize();

        let mqtt_client = mqtt_channel_client::Client::new(
            create_options,
            mqtt_channel_client::ClientConfigBuilder::default()
                .channel_size(128)
                .build()
                .unwrap(),
        )
        .expect("MQTT client should be created");

        let connect_options =
            mqtt_channel_client::paho_mqtt::connect_options::ConnectOptionsBuilder::new()
                .clean_session(true)
                .automatic_reconnect(Duration::from_secs(1), Duration::from_secs(5))
                .keep_alive_interval(Duration::from_secs(5))
                .user_name(&self.username)
                .password(&self.password)
                .finalize();

        mqtt_client
            .start(connect_options)
            .await
            .expect("MQTT client should be started");

        mqtt_client.subscribe(
            mqtt_channel_client::SubscriptionBuilder::default()
                .topic(self.topic.clone())
                .qos_exactly_once()
                .build()
                .unwrap(),
        );

        mqtt_client
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }
}

pub fn send_json<T: serde::Serialize>(
    client: &mqtt_channel_client::Client,
    topic: &str,
    s: &T,
) -> mqtt_channel_client::Result<()> {
    let data = serde_json::to_vec(s).unwrap();
    client.send(mqtt_channel_client::paho_mqtt::Message::new(topic, data, 2))
}
