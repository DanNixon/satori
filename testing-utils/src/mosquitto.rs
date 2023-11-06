use crate::PodmanDriver;
use std::io::Write;

pub struct MosquittoDriver {
    podman: PodmanDriver,
    port: u16,
}

impl Default for MosquittoDriver {
    fn default() -> Self {
        let temp_config_file = tempfile::NamedTempFile::new().unwrap();
        temp_config_file
            .as_file()
            .write_all(
                b"allow_anonymous true\n\
                listener 1883\n\
                ",
            )
            .unwrap();

        let port = rand::random::<u16>() % 1000 + 8000;

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

        Self { podman, port }
    }
}

impl MosquittoDriver {
    pub fn stop(&self) {
        self.podman.stop();
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn address(&self) -> String {
        format!("tcp://localhost:{}", self.port)
    }
}
