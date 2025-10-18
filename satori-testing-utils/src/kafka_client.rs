use rdkafka::{
    Message as KafkaMessage,
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    producer::{FutureProducer, FutureRecord},
};
use std::time::Duration;
use tracing::info;

pub struct TestKafkaClient {
    producer: FutureProducer,
    consumer: StreamConsumer,
    topic: String,
}

impl TestKafkaClient {
    pub async fn new(kafka_port: u16, topic: &str) -> Self {
        let brokers = format!("localhost:{}", kafka_port);
        
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("message.timeout.ms", "5000")
            .create()
            .expect("Failed to create Kafka producer");

        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &brokers)
            .set("group.id", "test-consumer")
            .set("enable.auto.commit", "true")
            .set("auto.offset.reset", "earliest")
            .create()
            .expect("Failed to create Kafka consumer");

        consumer
            .subscribe(&[topic])
            .expect("Failed to subscribe to topic");

        info!("Test Kafka client subscribed to topic: {}", topic);

        Self {
            producer,
            consumer,
            topic: topic.to_string(),
        }
    }

    pub async fn wait_for_message(&mut self, timeout: Duration) -> Result<TestMessage, String> {
        let start = std::time::Instant::now();
        
        while start.elapsed() < timeout {
            match tokio::time::timeout(Duration::from_millis(100), self.consumer.recv()).await {
                Ok(Ok(message)) => {
                    if let Some(payload) = message.payload() {
                        let payload_str = std::str::from_utf8(payload)
                            .map_err(|e| format!("Failed to decode message: {}", e))?;
                        
                        return Ok(TestMessage {
                            topic: message.topic().to_string(),
                            payload: payload_str.to_string(),
                        });
                    }
                }
                Ok(Err(e)) => {
                    return Err(format!("Consumer error: {}", e));
                }
                Err(_) => {
                    // Timeout, continue polling
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
        
        Err(format!("Timeout waiting for message after {:?}", timeout))
    }
}

pub struct TestMessage {
    pub topic: String,
    pub payload: String,
}

impl TestMessage {
    pub fn try_payload_str(&self) -> Result<&str, ()> {
        Ok(&self.payload)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::RedpandaDriver;

    #[tokio::test]
    #[ignore] // Requires Redpanda to be running
    async fn test_kafka_client() {
        let redpanda = RedpandaDriver::default();
        redpanda.wait_for_ready().await;

        let topic = "test-topic";
        let mut client = TestKafkaClient::new(redpanda.kafka_port(), topic).await;

        // Give consumer time to subscribe
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Send a test message using the producer
        let test_payload = "Hello Kafka!";
        let record: FutureRecord<(), &str> = FutureRecord::to(topic)
            .payload(test_payload);
        
        client.producer.send(record, Duration::from_secs(0)).await.unwrap();

        // Wait for the message
        let msg = client.wait_for_message(Duration::from_secs(5)).await.unwrap();
        assert_eq!(msg.topic, topic);
        assert_eq!(msg.payload, test_payload);
    }
}
