use std::process::Command;

pub struct PodmanDriver {
    container_id: String,
}

impl PodmanDriver {
    pub fn new(image: &str, ports: &[&str], env_vars: &[&str], args: &[&str]) -> Self {
        // Build Podman args
        let mut podman_args = vec!["run".to_string(), "--detach".to_string()];

        for port in ports.iter() {
            podman_args.push("-p".to_string());
            podman_args.push(port.to_string());
        }

        for var in env_vars.iter() {
            podman_args.push("-e".to_string());
            podman_args.push(var.to_string());
        }

        podman_args.push(image.to_string());

        for arg in args.iter() {
            podman_args.push(arg.to_string());
        }

        // Start the container
        let container_start = Command::new("podman").args(podman_args).output().unwrap();
        println!("Container start: {:?}", container_start);

        if container_start.status.success() && container_start.status.code().unwrap() == 0 {
            let container_id = String::from_utf8(container_start.stdout)
                .unwrap()
                .trim()
                .to_string();
            println!("Container ID: {container_id}");

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
        println!("Container stop: {:?}", container_stop);

        // Remove the container
        let container_remove = Command::new("podman")
            .args(vec!["rm", &self.container_id])
            .output()
            .unwrap();
        println!("Container remove: {:?}", container_remove);
    }
}
