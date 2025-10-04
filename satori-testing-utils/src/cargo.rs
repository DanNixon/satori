use nix::{
    sys::signal::{self, Signal},
    unistd::{self, Pid},
};
use std::{
    path::PathBuf,
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    task::JoinHandle,
};
use tracing::{debug, info};

type SharedPid = Arc<Mutex<Option<Pid>>>;

pub struct CargoBinaryRunner {
    pid: SharedPid,
    handle: Option<JoinHandle<()>>,
}

impl CargoBinaryRunner {
    pub fn new(binary: String, args: Vec<String>, env: Vec<(String, String)>) -> Self {
        let pid = SharedPid::default();

        let mut workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        workspace_dir.pop();

        let handle = {
            let name = binary.clone();

            let pid = pid.clone();

            Some(tokio::spawn(async move {
                let mut cargo_process = unsafe {
                    Command::new("cargo")
                        .current_dir(workspace_dir)
                        .envs(env)
                        // Run the specified binary
                        .arg("run")
                        .arg("--bin")
                        .arg(binary)
                        // In release mode
                        .arg("--release")
                        // With the specified arguments
                        .arg("--")
                        .args(args)
                        // Do nothing with stdin
                        .stdin(Stdio::null())
                        // Capture stdout and stderr
                        .stdout(Stdio::piped())
                        .stderr(Stdio::piped())
                        // Call setsid, required for correct exit signal handling
                        .pre_exec(|| {
                            unistd::setsid()?;
                            Ok(())
                        })
                        .spawn()
                        .expect("process should be started")
                };

                // Get and store the PID
                *pid.lock().unwrap() = Some(Pid::from_raw(
                    cargo_process.id().expect("process should have a PID") as i32,
                ));

                let stdout = cargo_process.stdout.take().unwrap();
                let stderr = cargo_process.stderr.take().unwrap();

                let mut stdout_reader = BufReader::new(stdout).lines();
                let mut stderr_reader = BufReader::new(stderr).lines();

                loop {
                    tokio::select! {
                        line = stdout_reader.next_line() => {
                            match line {
                                Ok(Some(line)) => debug!("{name} stdout: {line}"),
                                Err(_) => break,
                                _ => (),
                            }
                        }
                        line = stderr_reader.next_line() => {
                            match line {
                                Ok(Some(line)) => debug!("{name} stderr: {line}"),
                                Err(_) => break,
                                _ => (),
                            }
                        }
                        // Wait for process to exit
                        result = cargo_process.wait() => {
                            info!("{name} cargo exited, ok={}", result.is_ok());
                            *pid.lock().unwrap() = None;
                            break;
                        }
                    }
                }
            }))
        };

        Self { pid, handle }
    }

    pub fn stop(&self) {
        const EXIT_SIGNAL: Signal = Signal::SIGINT;

        // Request process to terminate
        info!("Sending {} to process", EXIT_SIGNAL);
        if let Some(pid) = *self.pid.lock().unwrap() {
            signal::kill(pid, EXIT_SIGNAL).unwrap();
        }
    }

    pub async fn wait(&mut self) {
        // Wait for process to exit
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
    }
}

impl Drop for CargoBinaryRunner {
    fn drop(&mut self) {
        self.stop();
    }
}
