use super::StreamerConfig;
use crate::Event;
use nix::{
    sys::signal::{self, Signal},
    unistd::{self, Pid},
};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
};
use tokio::{process::Command, sync::broadcast::Sender, task::JoinHandle};
use tracing::{debug, info};

const FFMPEG_EXIT_SIGNAL: Signal = Signal::SIGINT;

const PLAYLIST_FILENAME: &str = "stream.m3u8";

struct Inner {
    events_tx: Sender<Event>,
    config: StreamerConfig,
    destination: PathBuf,
    frame_file: PathBuf,

    ffmpeg_pid: Mutex<Option<Pid>>,
}

pub(crate) struct Streamer {
    inner: Arc<Inner>,
}

impl Streamer {
    pub(crate) fn new(
        events_tx: Sender<Event>,
        config: StreamerConfig,
        destination: &Path,
        frame_file: &Path,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                events_tx,
                config,
                destination: destination.to_owned(),
                frame_file: frame_file.to_owned(),
                ffmpeg_pid: Default::default(),
            }),
        }
    }

    #[allow(clippy::async_yields_async)]
    #[tracing::instrument(skip_all)]
    pub(crate) async fn start(&self) -> JoinHandle<()> {
        let inner = self.inner.clone();

        tokio::spawn(async move {
            // Start ffmpeg as a child process
            let mut ffmpeg_process = unsafe {
                Command::new("ffmpeg")
                    // Always overwrite files
                    .arg("-y")
                    // Stream config
                    .args(&inner.config.ffmpeg_input_args)
                    .arg("-i")
                    .arg(inner.config.url.to_string())
                    .arg("-c:v")
                    .arg("copy")
                    .arg("-c:a")
                    .arg("copy")
                    // HLS output stream
                    .arg("-f")
                    .arg("hls")
                    .arg("-hls_time")
                    .arg(inner.config.hls_segment_time.to_string())
                    .arg("-hls_list_size")
                    .arg(inner.config.hls_retained_segment_count.to_string())
                    .arg("-hls_flags")
                    .arg("append_list+delete_segments")
                    .arg("-hls_segment_filename")
                    .arg(
                        inner
                            .destination
                            .join(satori_common::SEGMENT_FILENAME_FORMAT),
                    )
                    .arg("-strftime")
                    .arg("1")
                    .arg(inner.destination.join(PLAYLIST_FILENAME))
                    // Output preview frames as JPEG
                    .arg("-vf")
                    .arg("fps=1")
                    .arg("-update")
                    .arg("1")
                    .arg(&inner.frame_file)
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
            *inner
                .ffmpeg_pid
                .lock()
                .expect("ffmpeg PID lock acquire should not fail") = Some(Pid::from_raw(
                ffmpeg_process
                    .id()
                    .expect("ffmpeg process should have a PID") as i32,
            ));
            info!("ffmpeg PID: {:?}", inner.ffmpeg_pid);

            // Wait for ffmpeg process to exit
            let result = ffmpeg_process.wait().await;
            info!("ffmpeg exited, ok={}", result.is_ok());
            *inner
                .ffmpeg_pid
                .lock()
                .expect("ffmpeg PID lock acquire should not fail") = None;

            // Signal app shutdown
            inner.events_tx.send(Event::Shutdown(Err(()))).unwrap();
        })
    }

    #[tracing::instrument(skip_all)]
    pub(crate) async fn stop(&self) {
        // Request ffmpeg to terminate
        info!("Sending {} to ffmpeg process", FFMPEG_EXIT_SIGNAL);
        if let Some(ffmpeg_pid) = *self
            .inner
            .ffmpeg_pid
            .lock()
            .expect("ffmpeg PID lock acquire should not fail")
        {
            signal::kill(ffmpeg_pid, FFMPEG_EXIT_SIGNAL).unwrap();
        }
    }
}
