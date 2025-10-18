use crate::podman::PodmanDriver;
use std::time::Duration;
use tracing::info;

pub struct RedpandaDriver {
    driver: PodmanDriver,
}

impl Default for RedpandaDriver {
    fn default() -> Self {
        let driver = PodmanDriver::new(
            "redpanda".to_string(),
            "docker.redpanda.com/redpandadata/redpanda:latest".to_string(),
            vec![
                "redpanda".to_string(),
                "start".to_string(),
                "--smp".to_string(),
                "1".to_string(),
                "--overprovisioned".to_string(),
                "--kafka-addr".to_string(),
                "internal://0.0.0.0:9092,external://0.0.0.0:19092".to_string(),
                "--advertise-kafka-addr".to_string(),
                "internal://redpanda:9092,external://localhost:19092".to_string(),
                "--pandaproxy-addr".to_string(),
                "internal://0.0.0.0:8082,external://0.0.0.0:18082".to_string(),
                "--advertise-pandaproxy-addr".to_string(),
                "internal://redpanda:8082,external://localhost:18082".to_string(),
                "--schema-registry-addr".to_string(),
                "internal://0.0.0.0:8081,external://0.0.0.0:18081".to_string(),
                "--rpc-addr".to_string(),
                "redpanda:33145".to_string(),
                "--advertise-rpc-addr".to_string(),
                "redpanda:33145".to_string(),
                "--mode".to_string(),
                "dev-container".to_string(),
            ],
            vec![
                ("19092".to_string(), "19092".to_string()), // Kafka port
                ("18081".to_string(), "18081".to_string()), // Schema registry port
                ("18082".to_string(), "18082".to_string()), // HTTP Proxy port
            ],
        );

        Self { driver }
    }
}

impl RedpandaDriver {
    pub async fn wait_for_ready(&self) {
        info!("Waiting for Redpanda to be ready");
        tokio::time::sleep(Duration::from_secs(10)).await;
        info!("Redpanda should be ready");
    }

    pub fn kafka_port(&self) -> u16 {
        self.driver.get_host_port("19092").unwrap()
    }

    pub fn schema_registry_port(&self) -> u16 {
        self.driver.get_host_port("18081").unwrap()
    }

    pub fn http_proxy_port(&self) -> u16 {
        self.driver.get_host_port("18082").unwrap()
    }
}
