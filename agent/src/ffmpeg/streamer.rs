use crate::{config::Config, jpeg_frame_decoder::JpegFrameDecoder};
use futures::StreamExt;
use nix::{
    sys::signal::{self, Signal},
    unistd::{self, Pid},
};
use std::{
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::broadcast,
    task::JoinHandle,
};
use tokio_util::codec::FramedRead;
use tracing::{debug, error, info, warn};

const HLS_PLAYLIST_FILENAME: &str = "stream.m3u8";

pub(crate) struct Streamer {
    config: Config,
    terminate: Arc<Mutex<bool>>,
    ffmpeg_pid: Arc<Mutex<Option<Pid>>>,
    handle: Option<JoinHandle<()>>,
    jpeg_tx: broadcast::Sender<bytes::Bytes>,
}

impl Streamer {
    pub(crate) fn new(config: Config) -> Self {
        let (tx, _) = broadcast::channel(8);

        Self {
            config,
            terminate: Arc::new(Mutex::new(false)),
            ffmpeg_pid: Default::default(),
            handle: None,
            jpeg_tx: tx,
        }
    }

    pub(crate) fn jpeg_subscribe(&self) -> broadcast::Receiver<bytes::Bytes> {
        self.jpeg_tx.subscribe()
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn start(&mut self) {
        let config = self.config.clone();
        let ffmpeg_pid = self.ffmpeg_pid.clone();
        let terminate = self.terminate.clone();
        let jpeg_tx = self.jpeg_tx.clone();

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
                        .arg("-f")
                        .arg("image2")
                        .arg("-update")
                        .arg("1")
                        .arg("pipe:1")
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
                        .expect("ffmpeg process should be started")
                };
                debug!("ffmpeg process: {:?}", ffmpeg_process);

                // Get and store the ffmpeg PID
                *ffmpeg_pid.lock().unwrap() = {
                    let pid = Pid::from_raw(
                        ffmpeg_process
                            .id()
                            .expect("ffmpeg process should have a PID")
                            as i32,
                    );
                    info!("ffmpeg PID: {:?}", pid);
                    Some(pid)
                };

                // Increment ffmpeg invocation count
                metrics::counter!(crate::METRIC_FFMPEG_INVOCATIONS, 1);

                let stdout = ffmpeg_process.stdout.take().unwrap();
                let mut stdout_frame = FramedRead::new(stdout, JpegFrameDecoder);

                let stderr = ffmpeg_process.stderr.take().unwrap();
                let mut stderr_reader = BufReader::new(stderr).lines();

                loop {
                    tokio::select! {
                        // Handle JPEG data via stdout
                        Some(frame) = stdout_frame.next() => {
                            match frame {
                                Ok(frame) => {
                                    debug!("Got JPEG frame ({} bytes)", frame.len());
                                    if let Err(e ) = jpeg_tx.send(frame) {
                                        error!("JPEG frame channel error: {}", e);
                                    }
                                }
                                Err(e) => error!("ffmpeg stdout frame errror: {:?}", e),
                            }
                        }
                        // Output stderr to log with prefix
                        line = stderr_reader.next_line() => {
                            match line {
                                Ok(Some(line)) => info!("ffmpeg stderr: {line}"),
                                Err(e) => {
                                    warn!("ffmpeg stderr closed: {e}");
                                    break;
                                },
                                _ => (),
                            }
                        }
                        // Wait for ffmpeg process to exit
                        result = ffmpeg_process.wait() => {
                            info!("ffmpeg exited, ok={}", result.is_ok());
                            *ffmpeg_pid.lock().unwrap() = None;
                            break;
                        }
                    }
                }

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
