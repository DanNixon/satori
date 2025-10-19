use crate::ThrottledErrorLogger;
use rdkafka::{
    Message as KafkaMessage,
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    producer::{FutureProducer, FutureRecord},
};
use serde::Deserialize;
use std::time::Duration;
use tracing::{error, info, warn};

#[derive(Debug, Deserialize, Clone)]
pub struct KafkaConfig {
    brokers: String,
    topic: String,

    #[serde(default)]
    group_id: Option<String>,
}

pub struct KafkaProducer {
    producer: FutureProducer,
    topic: String,
}

impl From<KafkaConfig> for KafkaProducer {
    fn from(config: KafkaConfig) -> Self {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &config.brokers)
            .set("message.timeout.ms", "5000")
            .create()
            .expect("Failed to create Kafka producer");

        Self {
            producer,
            topic: config.topic,
        }
    }
}

impl KafkaProducer {
    pub fn topic(&self) -> &str {
        &self.topic
    }

    pub async fn send_json<T: serde::Serialize + Sync>(&self, payload: &T) {
        let payload = serde_json::to_vec(payload).expect("Message should be serialized to JSON");

        let record: FutureRecord<(), Vec<u8>> = FutureRecord::to(&self.topic).payload(&payload);

        if let Err((e, _)) = self.producer.send(record, Duration::from_secs(0)).await {
            error!("Failed to send message: {:?}", e);
        }
    }
}

pub struct KafkaConsumer {
    consumer: StreamConsumer,
    poll_error_logger: ThrottledErrorLogger<String>,
    topic: String,
}

impl From<KafkaConfig> for KafkaConsumer {
    fn from(config: KafkaConfig) -> Self {
        let group_id = config
            .group_id
            .unwrap_or_else(|| "satori-consumer".to_string());

        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &config.brokers)
            .set("group.id", &group_id)
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .create()
            .expect("Failed to create Kafka consumer");

        consumer
            .subscribe(&[&config.topic])
            .expect("Failed to subscribe to topic");

        info!("Subscribed to Kafka topic: {}", config.topic);

        Self {
            consumer,
            poll_error_logger: ThrottledErrorLogger::new(Duration::from_secs(5)),
            topic: config.topic,
        }
    }
}

impl KafkaConsumer {
    pub async fn poll(&mut self) -> Option<Vec<u8>> {
        match tokio::time::timeout(Duration::from_millis(100), self.consumer.recv()).await {
            Ok(Ok(message)) => {
                message.payload().map(|payload| payload.to_vec())
            }
            Ok(Err(e)) => {
                if let Some(e) = self.poll_error_logger.log(format!("{e:?}")) {
                    warn!("Kafka consumer error: {}", e);
                }
                None
            }
            Err(_) => None, // Timeout, no message available
        }
    }

    pub fn topic(&self) -> &str {
        &self.topic
    }
}

pub trait PayloadExt {
    fn try_payload_str(&self) -> Result<&str, std::str::Utf8Error>;
    fn try_payload_from_json<'a, T: serde::Deserialize<'a>>(&'a self) -> serde_json::Result<T>;
}

impl PayloadExt for Vec<u8> {
    fn try_payload_str(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self)
    }

    fn try_payload_from_json<'a, T: serde::Deserialize<'a>>(&'a self) -> serde_json::Result<T> {
        serde_json::from_slice(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use satori_testing_utils::RedpandaDriver;

    #[tokio::test]
    async fn producer_consumer_test() {
        let redpanda = RedpandaDriver::default();
        redpanda.wait_for_ready().await;

        let topic = "test-topic";

        let producer_config = KafkaConfig {
            brokers: format!("localhost:{}", redpanda.kafka_port()),
            topic: topic.to_string(),
            group_id: None,
        };

        let consumer_config = KafkaConfig {
            brokers: format!("localhost:{}", redpanda.kafka_port()),
            topic: topic.to_string(),
            group_id: Some("test-consumer".to_string()),
        };

        let producer: KafkaProducer = producer_config.into();
        let mut consumer: KafkaConsumer = consumer_config.into();

        let test_message = "Hello, Kafka!";
        producer.send_json(&test_message).await;

        // Give some time for the message to be delivered
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Poll for the message
        let mut received = false;
        for _ in 0..10 {
            if let Some(payload) = consumer.poll().await {
                let msg: String = payload.try_payload_from_json().unwrap();
                assert_eq!(msg, test_message);
                received = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }

        assert!(received, "Should have received the message");
    }
}
