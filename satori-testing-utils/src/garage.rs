use crate::PodmanDriver;
use s3::{Bucket, BucketConfiguration, Region, creds::Credentials};
use std::{io::Write, time::Duration};
use tempfile::NamedTempFile;

pub struct GarageDriver {
    _podman: PodmanDriver,
    _config_file: NamedTempFile,

    endpoint: String,

    key_id: String,
    secret_key: String,
}

impl Default for GarageDriver {
    fn default() -> Self {
        let key_id = "garageadmin".to_string();
        let secret_key = "garageadminsecret".to_string();

        let port = rand::random::<u16>() % 1000 + 8000;

        let rpc_secret: String = (0..32)
            .map(|_| format!("{:02x}", rand::random::<u8>()))
            .collect();

        let mut config_file = NamedTempFile::new().expect("Failed to create temporary config file");
        let config_content = format!(
            r#"metadata_dir = "/tmp/meta"
data_dir = "/tmp/data"
db_engine = "sqlite"

replication_factor = 1

rpc_bind_addr = "[::]:3901"
rpc_secret = "{rpc_secret}"

[s3_api]
s3_region = "garage"
api_bind_addr = "[::]:3900"
"#
        );
        config_file
            .write_all(config_content.as_bytes())
            .expect("Failed to write garage config file");
        config_file
            .flush()
            .expect("Failed to flush garage config file");

        let config_path = config_file.path().display().to_string();

        let podman = PodmanDriver::new(
            "docker.io/dxflrs/garage:v2.4.1",
            &[&format!("{port}:3900")],
            &[
                &format!("GARAGE_DEFAULT_ACCESS_KEY={key_id}"),
                &format!("GARAGE_DEFAULT_SECRET_KEY={secret_key}"),
                "GARAGE_ALLOW_WORLD_READABLE_SECRETS=true",
                "GARAGE_CONFIG_FILE=/garage.toml",
            ],
            &[&format!("{config_path}:/garage.toml:ro,z")],
            &[
                "/garage",
                "-c",
                "/garage.toml",
                "server",
                "--single-node",
                "--default-access-key",
            ],
        );

        let endpoint = format!("http://localhost:{port}");

        Self {
            _podman: podman,
            _config_file: config_file,
            endpoint,
            key_id,
            secret_key,
        }
    }
}

impl GarageDriver {
    pub fn endpoint(&self) -> String {
        self.endpoint.clone()
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    pub fn secret_key(&self) -> &str {
        &self.secret_key
    }

    pub fn region(&self) -> &str {
        "garage"
    }

    pub async fn wait_for_ready(&self) {
        crate::wait_for_url(&self.endpoint, Duration::from_secs(600))
            .await
            .expect("Garage should be running");
    }

    pub fn set_credential_env_vars(&self) {
        unsafe {
            std::env::set_var("AWS_ACCESS_KEY_ID", self.key_id.clone());
            std::env::set_var("AWS_SECRET_ACCESS_KEY", self.secret_key.clone());
            std::env::set_var("AWS_REGION", self.region());
            std::env::set_var("AWS_DEFAULT_REGION", self.region());
        }
    }

    pub async fn create_bucket(&self, name: &str) -> Box<Bucket> {
        Bucket::create_with_path_style(
            name,
            Region::Custom {
                region: self.region().to_string(),
                endpoint: self.endpoint(),
            },
            Credentials::new(Some(&self.key_id), Some(&self.secret_key), None, None, None).unwrap(),
            BucketConfiguration::default(),
        )
        .await
        .unwrap()
        .bucket
    }
}
