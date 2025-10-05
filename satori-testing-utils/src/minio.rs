use crate::PodmanDriver;
use s3::{Bucket, BucketConfiguration, Region, creds::Credentials};
use std::time::Duration;

pub struct MinioDriver {
    _podman: PodmanDriver,

    endpoint: String,

    key_id: String,
    secret_key: String,
}

impl Default for MinioDriver {
    fn default() -> Self {
        let key_id = "minioadmin".to_string();
        let secret_key = "minioadmin".to_string();

        let port = rand::random::<u16>() % 1000 + 8000;

        let podman = PodmanDriver::new(
            "docker.io/minio/minio",
            &[&format!("{port}:9000")],
            &[
                &format!("MINIO_ACCESS_KEY={key_id}"),
                &format!("MINIO_SECRET_KEY={secret_key}"),
            ],
            &[],
            &["server", "/data"],
        );

        let endpoint = format!("http://localhost:{port}");

        Self {
            _podman: podman,
            endpoint,
            key_id,
            secret_key,
        }
    }
}

impl MinioDriver {
    pub fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    pub async fn wait_for_ready(&self) {
        crate::wait_for_url(&self.endpoint, Duration::from_secs(600))
            .await
            .expect("Minio should be running");
    }

    pub fn set_credential_env_vars(&self) {
        unsafe {
            std::env::set_var("AWS_ACCESS_KEY_ID", self.key_id.clone());
            std::env::set_var("AWS_SECRET_ACCESS_KEY", self.secret_key.clone());
        }
    }

    pub async fn create_bucket(&self, name: &str) -> Box<Bucket> {
        Bucket::create_with_path_style(
            name,
            Region::Custom {
                region: "".to_string(),
                endpoint: self.endpoint(),
            },
            Credentials::default().unwrap(),
            BucketConfiguration::default(),
        )
        .await
        .unwrap()
        .bucket
    }
}
