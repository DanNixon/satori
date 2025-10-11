use axum::{
    Router,
    extract::Path,
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Utc};
use m3u8_rs::{MediaPlaylist, MediaSegment};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    time::Duration,
};
use tokio::{net::TcpListener, task::JoinHandle};

pub struct DummyStreamParams {
    start_time: DateTime<Utc>,
    segment_duration: Duration,
    segment_count: usize,
}

impl DummyStreamParams {
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
}

impl From<DummyStreamParams> for MediaPlaylist {
    fn from(params: DummyStreamParams) -> Self {
        let segment_duration = params.segment_duration.as_secs();
        let target_duration = segment_duration * (params.segment_count as u64);

        let segments = (0..params.segment_count)
            .map(|i| {
                let segment_timestamp =
                    params.start_time + (params.segment_duration * i.try_into().unwrap());

                let filename = segment_timestamp.format(satori_common::SEGMENT_FILENAME_FORMAT);

                MediaSegment {
                    uri: filename.to_string(),
                    duration: segment_duration as f32,
                    ..Default::default()
                }
            })
            .collect();

        Self {
            target_duration,
            segments,
            ..Default::default()
        }
    }
}

pub struct DummyHlsServer {
    handle: Option<JoinHandle<()>>,
    stream_address: String,
}

impl DummyHlsServer {
    pub async fn new(name: String, playlist: MediaPlaylist) -> Self {
        let app = Router::new().route(
            "/{filename}",
            get(move |Path(filename): Path<String>| handler(name, filename, playlist.clone())),
        );

        let port = rand::random::<u16>() % 1000 + 8000;
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), port);

        let listener = TcpListener::bind(&address)
            .await
            .unwrap_or_else(|_| panic!("tcp listener should bind to {address}"));

        let handle = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));

        let stream_address = format!("http://localhost:{port}/stream.m3u8");

        Self {
            handle,
            stream_address,
        }
    }

    pub async fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
            let _ = handle.await;
        }
    }

    pub fn stream_address(&self) -> String {
        self.stream_address.clone()
    }
}

async fn handler(name: String, filename: String, playlist: MediaPlaylist) -> Response {
    if filename == "stream.m3u8" {
        let mut s = Vec::new();
        playlist.write_to(&mut s).unwrap();
        s.into_response()
    } else if filename.ends_with(".ts") {
        let s = format!("Dummy MPEG-TS segment for dummy HLS stream \"{name}\"\n{filename}\n");
        Html(s).into_response()
    } else {
        (StatusCode::NOT_FOUND, "Not found").into_response()
    }
}
