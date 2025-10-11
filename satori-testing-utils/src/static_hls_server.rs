use crate::PodmanDriver;
use chrono::{DateTime, Utc};
use m3u8_rs::{MediaPlaylist, MediaSegment};
use std::{fs, path::Path, time::Duration};

pub struct StaticHlsServerParams {
    start_time: DateTime<Utc>,
    segment_duration: Duration,
    segment_count: usize,
}

impl StaticHlsServerParams {
    pub fn new(start_time: &str, segment_duration: Duration, segment_count: usize) -> Self {
        Self {
            start_time: DateTime::parse_from_rfc3339(start_time).unwrap().into(),
            segment_duration,
            segment_count,
        }
    }

    pub fn new_ending_now(segment_duration: Duration, segment_count: usize) -> Self {
        let start_time = Utc::now() - (segment_duration * segment_count as u32);

        Self {
            start_time,
            segment_duration,
            segment_count,
        }
    }

    fn generate_playlist(&self) -> MediaPlaylist {
        let segment_duration = self.segment_duration.as_secs();
        let target_duration = segment_duration * (self.segment_count as u64);

        let segments = (0..self.segment_count)
            .map(|i| {
                let segment_timestamp =
                    self.start_time + (self.segment_duration * i.try_into().unwrap());

                let filename = segment_timestamp.format(satori_common::SEGMENT_FILENAME_FORMAT);

                MediaSegment {
                    uri: filename.to_string(),
                    duration: segment_duration as f32,
                    ..Default::default()
                }
            })
            .collect();

        MediaPlaylist {
            target_duration,
            segments,
            ..Default::default()
        }
    }

    fn generate_segment_files(&self, dir: &Path, name: &str) -> std::io::Result<()> {
        for i in 0..self.segment_count {
            let segment_timestamp =
                self.start_time + (self.segment_duration * i.try_into().unwrap());
            let filename = segment_timestamp.format(satori_common::SEGMENT_FILENAME_FORMAT);
            let segment_path = dir.join(format!("{}", filename));

            let content =
                format!("Dummy MPEG-TS segment for static HLS stream \"{name}\"\n{filename}\n");
            fs::write(segment_path, content)?;
        }
        Ok(())
    }
}

pub struct StaticHlsServer {
    _podman: PodmanDriver,
    _temp_dir: tempfile::TempDir,
    stream_address: String,
}

impl StaticHlsServer {
    pub fn new(name: String, params: StaticHlsServerParams) -> Self {
        // Create temporary directory for HLS files
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Generate playlist
        let playlist = params.generate_playlist();
        let mut playlist_bytes = Vec::new();
        playlist
            .write_to(&mut playlist_bytes)
            .expect("Failed to write playlist");
        fs::write(temp_path.join("stream.m3u8"), playlist_bytes)
            .expect("Failed to write playlist file");

        // Generate segment files
        params
            .generate_segment_files(temp_path, &name)
            .expect("Failed to generate segment files");

        // Start nginx container to serve the files
        let port = rand::random::<u16>() % 1000 + 8000;
        let podman = PodmanDriver::new(
            "docker.io/library/nginx:alpine",
            &[&format!("{port}:80")],
            &[],
            &[&format!("{}:/usr/share/nginx/html:ro", temp_path.display())],
            &[],
        );

        let stream_address = format!("http://localhost:{port}/stream.m3u8");

        Self {
            _podman: podman,
            _temp_dir: temp_dir,
            stream_address,
        }
    }

    pub fn stream_address(&self) -> String {
        self.stream_address.clone()
    }
}
