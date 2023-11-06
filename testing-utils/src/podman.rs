use std::process::Command;
use tracing::{debug, info};

pub struct PodmanDriver {
    container_id: String,
}

impl PodmanDriver {
    pub fn new(
        image: &str,
        ports: &[&str],
        env_vars: &[&str],
        volumes: &[&str],
        args: &[&str],
    ) -> Self {
        // Build Podman args
        let mut podman_args = vec![
            "run".to_string(),
            "--detach".to_string(),
            "--rm".to_string(),
        ];

        for port in ports.iter() {
            podman_args.push("-p".to_string());
            podman_args.push(port.to_string());
        }

        for var in env_vars.iter() {
            podman_args.push("-e".to_string());
            podman_args.push(var.to_string());
        }

        for volume in volumes.iter() {
            podman_args.push("-v".to_string());
            podman_args.push(volume.to_string());
        }

        podman_args.push(image.to_string());

        for arg in args.iter() {
            podman_args.push(arg.to_string());
        }

        // Start the container
        let container_start = Command::new("podman").args(podman_args).output().unwrap();
        debug!("Container start: {:?}", container_start);

        if container_start.status.success() && container_start.status.code().unwrap() == 0 {
            let container_id = String::from_utf8(container_start.stdout)
                .unwrap()
                .trim()
                .to_string();
            info!("Container ID: {container_id}");

            Self { container_id }
        } else {
            panic!("Failed to start container");
        }
    }

    pub fn stop(&self) {
        // Stop the container
        let container_stop = Command::new("podman")
            .args(vec!["stop", &self.container_id])
            .output()
            .unwrap();
        debug!("Container stop: {:?}", container_stop);
    }
}

impl Drop for PodmanDriver {
    fn drop(&mut self) {
        self.stop();
    }
}
