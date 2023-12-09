use crate::PodmanDriver;
use std::io::Write;

pub struct MosquittoDriver {
    _podman: PodmanDriver,
    port: u16,
}

impl Default for MosquittoDriver {
    fn default() -> Self {
        let port = rand::random::<u16>() % 1000 + 8000;
        Self::with_port(port)
    }
}

impl MosquittoDriver {
    pub fn with_port(port: u16) -> Self {
        let temp_config_file = tempfile::NamedTempFile::new().unwrap();
        temp_config_file
            .as_file()
            .write_all(
                b"allow_anonymous true\n\
                listener 1883\n\
                ",
            )
            .unwrap();

        let podman = PodmanDriver::new(
            "docker.io/library/eclipse-mosquitto",
            &[&format!("{port}:1883")],
            &[],
            &[&format!(
                "{}:/mosquitto/config/mosquitto.conf",
                temp_config_file.path().display()
            )],
            &[],
        );

        Self {
            _podman: podman,
            port,
        }
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn address(&self) -> String {
        format!("tcp://localhost:{}", self.port)
    }
}
