use crate::PodmanDriver;

pub struct MinioDriver {
    pub podman: PodmanDriver,
    pub endpoint: String,
}

impl Default for MinioDriver {
    fn default() -> Self {
        let port = rand::random::<u16>() % 1000 + 9000;

        let podman = PodmanDriver::new(
            "docker.io/minio/minio",
            &[&format!("{port}:9000")],
            &["MINIO_ACCESS_KEY=minioadmin", "MINIO_SECRET_KEY=minioadmin"],
            &["server", "/data"],
        );

        let endpoint = format!("http://localhost:{}", port);

        Self { podman, endpoint }
    }
}

impl MinioDriver {
    pub fn stop(&self) {
        self.podman.stop();
    }

    pub fn endpoint(&self) -> String {
        self.endpoint.clone()
    }
}
