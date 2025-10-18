use crate::podman::PodmanDriver;
use std::time::Duration;
use tracing::info;

pub struct RedpandaDriver {
    _driver: PodmanDriver,
    kafka_port: u16,
    schema_registry_port: u16,
    http_proxy_port: u16,
}

impl Default for RedpandaDriver {
    fn default() -> Self {
        let kafka_port = rand::random::<u16>() % 1000 + 19000;
        let schema_registry_port = kafka_port + 1;
        let http_proxy_port = kafka_port + 2;

        let driver = PodmanDriver::new(
            "docker.redpanda.com/redpandadata/redpanda:latest",
            &[
                &format!("{}:19092", kafka_port),
                &format!("{}:18081", schema_registry_port),
                &format!("{}:18082", http_proxy_port),
            ],
            &[],
            &[],
            &[
                "redpanda",
                "start",
                "--smp",
                "1",
                "--overprovisioned",
                "--kafka-addr",
                "internal://0.0.0.0:9092,external://0.0.0.0:19092",
                "--advertise-kafka-addr",
                &format!("internal://redpanda:9092,external://localhost:{}", kafka_port),
                "--pandaproxy-addr",
                "internal://0.0.0.0:8082,external://0.0.0.0:18082",
                "--advertise-pandaproxy-addr",
                &format!("internal://redpanda:8082,external://localhost:{}", http_proxy_port),
                "--schema-registry-addr",
                "internal://0.0.0.0:8081,external://0.0.0.0:18081",
                "--rpc-addr",
                "redpanda:33145",
                "--advertise-rpc-addr",
                "redpanda:33145",
                "--mode",
                "dev-container",
            ],
        );

        Self {
            _driver: driver,
            kafka_port,
            schema_registry_port,
            http_proxy_port,
        }
    }
}

impl RedpandaDriver {
    pub async fn wait_for_ready(&self) {
        info!("Waiting for Redpanda to be ready");
        tokio::time::sleep(Duration::from_secs(10)).await;
        info!("Redpanda should be ready");
    }

    pub fn kafka_port(&self) -> u16 {
        self.kafka_port
    }

    pub fn schema_registry_port(&self) -> u16 {
        self.schema_registry_port
    }

    pub fn http_proxy_port(&self) -> u16 {
        self.http_proxy_port
    }
}
