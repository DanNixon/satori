use crate::config::Config;
use kagiyama::prometheus::metrics::gauge::Gauge;
use nix::{
    sys::signal::{self, Signal},
    unistd::{self, Pid},
};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{process::Command, task::JoinHandle};
use tracing::{debug, info, warn};

const HLS_PLAYLIST_FILENAME: &str = "stream.m3u8";

pub(crate) struct Streamer {
    config: Config,
    frame_file: PathBuf,
    terminate: Arc<Mutex<bool>>,
    ffmpeg_pid: Arc<Mutex<Option<Pid>>>,
    ffmpeg_invocations_metric: Gauge,
    handle: Option<JoinHandle<()>>,
}

impl Streamer {
    pub(crate) fn new(config: Config, frame_file: &Path, ffmpeg_invocations_metric: Gauge) -> Self {
        Self {
            config,
            frame_file: frame_file.to_owned(),
            terminate: Arc::new(Mutex::new(false)),
            ffmpeg_pid: Default::default(),
            ffmpeg_invocations_metric,
            handle: None,
        }
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn start(&mut self) {
        let config = self.config.clone();
        let frame_file = self.frame_file.clone();
        let ffmpeg_pid = self.ffmpeg_pid.clone();
        let terminate = self.terminate.clone();
        let ffmpeg_invocations_metric = self.ffmpeg_invocations_metric.clone();

        self.handle = Some(tokio::spawn(async move {
            loop {
                // Start ffmpeg as a child process
                let mut ffmpeg_process = unsafe {
                    Command::new("ffmpeg")
                        // Always overwrite files
                        .arg("-y")
                        // Stream config
                        .args(&config.stream.ffmpeg_input_args)
                        .arg("-i")
                        .arg(config.stream.url.to_string())
                        .arg("-c:v")
                        .arg("copy")
                        .arg("-c:a")
                        .arg("copy")
                        // HLS output stream
                        .arg("-f")
                        .arg("hls")
                        .arg("-hls_time")
                        .arg(config.stream.hls_segment_time.to_string())
                        .arg("-hls_list_size")
                        .arg(config.stream.hls_retained_segment_count.to_string())
                        .arg("-hls_flags")
                        .arg("append_list+delete_segments")
                        .arg("-hls_segment_filename")
                        .arg(
                            config
                                .video_directory
                                .join(satori_common::SEGMENT_FILENAME_FORMAT),
                        )
                        .arg("-strftime")
                        .arg("1")
                        .arg(config.video_directory.join(HLS_PLAYLIST_FILENAME))
                        // Output preview frames as JPEG
                        .arg("-vf")
                        .arg("fps=1")
                        .arg("-update")
                        .arg("1")
                        .arg(&frame_file)
                        // Do nothing with stdin
                        .stdin(Stdio::null())
                        // Call setsid, required for correct exit signal handling
                        .pre_exec(|| {
                            unistd::setsid()?;
                            Ok(())
                        })
                        .spawn()
                        .expect("ffmpeg process should be started")
                };
                debug!("ffmpeg process: {:?}", ffmpeg_process);

                // Get and store the ffmpeg PID
                *ffmpeg_pid
                    .lock()
                    .expect("ffmpeg PID lock acquire should not fail") = Some(Pid::from_raw(
                    ffmpeg_process
                        .id()
                        .expect("ffmpeg process should have a PID") as i32,
                ));
                info!("ffmpeg PID: {:?}", ffmpeg_pid);

                // Increment ffmpeg invocation count
                ffmpeg_invocations_metric.inc();

                // Wait for ffmpeg process to exit
                let result = ffmpeg_process.wait().await;
                info!("ffmpeg exited, ok={}", result.is_ok());
                *ffmpeg_pid.lock().unwrap() = None;

                let expected_shutdown = *terminate.lock().unwrap();
                if expected_shutdown {
                    info!("Termination requested, not restarting ffmpeg");
                    break;
                } else {
                    warn!(
                        "ffmpeg exited unexpectedly, restarting in {:?}",
                        config.ffmpeg_restart_delay
                    );
                    tokio::time::sleep(config.ffmpeg_restart_delay).await;
                }
            }
        }));
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn stop(&mut self) {
        const FFMPEG_EXIT_SIGNAL: Signal = Signal::SIGINT;

        // Set terminate flag to ensure ffmpeg is not restarted
        *self.terminate.lock().unwrap() = true;

        // Request ffmpeg to terminate
        info!("Sending {} to ffmpeg process", FFMPEG_EXIT_SIGNAL);
        if let Some(ffmpeg_pid) = *self.ffmpeg_pid.lock().unwrap() {
            signal::kill(ffmpeg_pid, FFMPEG_EXIT_SIGNAL).unwrap();
        }

        // Wait for ffmpeg to exit
        if let Some(handle) = self.handle.take() {
            let _ = handle.await;
        }
    }
}
