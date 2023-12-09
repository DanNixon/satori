use rumqttc::{mqttbytes::v4::Publish, AsyncClient, Event, Incoming, MqttOptions};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{
    sync::{
        broadcast::{self, Receiver, Sender},
        watch,
    },
    task::JoinHandle,
};
use tracing::{error, info};

type MessageQueue = Arc<Mutex<VecDeque<Publish>>>;

pub struct TestMqttClient {
    handle: Option<JoinHandle<()>>,
    exit_tx: Sender<()>,

    client: AsyncClient,
    recevied_mqtt_messages: MessageQueue,
    message_rx: Receiver<Publish>,
}

impl TestMqttClient {
    pub async fn new(port: u16) -> Self {
        let mut options = MqttOptions::new("test-framework", "localhost", port);
        options.set_keep_alive(Duration::from_secs(5));

        let (client, mut event_loop) = AsyncClient::new(options, 10);

        let recevied_mqtt_messages = MessageQueue::default();

        let (exit_tx, mut exit_rx) = broadcast::channel(1);
        let (message_tx, message_rx) = broadcast::channel(16);
        let (connected_tx, mut connected_rx) = watch::channel(false);

        let handle = {
            let recevied_mqtt_messages = recevied_mqtt_messages.clone();

            Some(tokio::spawn(async move {
                loop {
                    tokio::select! {
                        event = event_loop.poll() => {
                            match event {
                                Ok(Event::Incoming(Incoming::ConnAck(_))) => {
                                    connected_tx.send(true).unwrap();
                                }
                                Ok(Event::Incoming(Incoming::Publish(msg))) => {
                                    info!("Received message: {:?}", msg);
                                    recevied_mqtt_messages.lock().unwrap().push_back(msg.clone());
                                    message_tx.send(msg).unwrap();
                                }
                                Err(e) => {
                                    error!("MQTT client error: {:?}", e);
                                }
                                _ => {}
                            }
                        }
                        _ = exit_rx.recv() => {
                            break;
                        }
                    }
                }
            }))
        };

        // Wait for the initial connection to complete
        connected_rx.wait_for(|connected| *connected).await.unwrap();

        Self {
            handle,
            exit_tx,
            client,
            recevied_mqtt_messages,
            message_rx,
        }
    }

    pub async fn stop(&mut self) {
        // Send exit signal to worker task
        self.exit_tx.send(()).unwrap();

        // Wait for worker task to exit
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
    }

    pub fn client(&self) -> &AsyncClient {
        &self.client
    }

    pub fn pop_message(&self) -> Option<Publish> {
        self.recevied_mqtt_messages.lock().unwrap().pop_front()
    }

    pub async fn wait_for_message(&mut self, timeout: Duration) -> Result<Publish, ()> {
        match tokio::time::timeout(timeout, self.message_rx.recv()).await {
            Ok(Ok(msg)) => Ok(msg),
            Ok(Err(_)) => Err(()),
            Err(_) => Err(()),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::MosquittoDriver;
    use satori_common::mqtt::PublishExt;

    #[ctor::ctor]
    fn init() {
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_max_level(tracing::Level::DEBUG)
            .init();
    }

    #[tokio::test]
    async fn basic() {
        let mosquitto = MosquittoDriver::default();

        let mut client = TestMqttClient::new(mosquitto.port()).await;

        client
            .client()
            .subscribe("test-1", rumqttc::QoS::AtLeastOnce)
            .await
            .unwrap();

        client
            .client()
            .subscribe("test-2", rumqttc::QoS::AtLeastOnce)
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        assert!(client.pop_message().is_none());

        client
            .client()
            .publish("test-1", rumqttc::QoS::AtLeastOnce, false, "Hello 1")
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        {
            let msg = client.pop_message();
            assert!(msg.is_some());
            let msg = msg.unwrap();
            assert_eq!(msg.topic, "test-1".to_string());
            assert_eq!(msg.try_payload_str().unwrap(), "Hello 1");
        }

        assert!(client.pop_message().is_none());

        client
            .client()
            .publish("test-2", rumqttc::QoS::AtLeastOnce, false, "Hello 2")
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        {
            let msg = client.pop_message();
            assert!(msg.is_some());
            let msg = msg.unwrap();
            assert_eq!(msg.topic, "test-2".to_string());
            assert_eq!(msg.try_payload_str().unwrap(), "Hello 2");
        }

        assert!(client.pop_message().is_none());

        tokio::time::sleep(Duration::from_millis(100)).await;

        client.stop().await;
    }

    #[tokio::test]
    async fn wait_for_message() {
        let mosquitto = MosquittoDriver::default();

        let mut client = TestMqttClient::new(mosquitto.port()).await;

        client
            .client()
            .subscribe("test-1", rumqttc::QoS::AtLeastOnce)
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(100)).await;

        client
            .client()
            .publish("test-1", rumqttc::QoS::AtLeastOnce, false, "Hello 1")
            .await
            .unwrap();

        let msg = client
            .wait_for_message(Duration::from_secs(5))
            .await
            .expect("a message should have been received");
        assert_eq!(msg.topic, "test-1".to_string());
        assert_eq!(msg.try_payload_str().unwrap(), "Hello 1");

        client.stop().await;
    }

    #[tokio::test]
    async fn wait_for_message_timeout() {
        let mosquitto = MosquittoDriver::default();

        let mut client = TestMqttClient::new(mosquitto.port()).await;

        client
            .client()
            .subscribe("test-1", rumqttc::QoS::AtLeastOnce)
            .await
            .unwrap();

        let msg = client.wait_for_message(Duration::from_secs(2)).await;
        assert!(msg.is_err());

        client.stop().await;
    }
}
