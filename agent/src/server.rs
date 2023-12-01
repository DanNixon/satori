use crate::config::Config;
use axum::{response::Html, routing::get, Router};
use std::{net::SocketAddr, path::PathBuf};
use tokio::{net::TcpListener, task::JoinHandle};
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

pub(super) struct Server {
    handle: Option<JoinHandle<()>>,
}

impl Server {
    pub(super) async fn new(address: SocketAddr, config: Config, frame_file: PathBuf) -> Self {
        let listener = TcpListener::bind(&address)
            .await
            .unwrap_or_else(|_| panic!("tcp listener should bind to {address}"));

        let app = Router::new()
            .route("/player", get(Html(include_str!("player.html"))))
            .nest_service("/frame.jpg", ServeFile::new(frame_file))
            .nest_service("/", ServeDir::new(config.video_directory));

        info!("Starting HTTP server on {}", address);
        let handle = Some(tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        }));

        Self { handle }
    }

    pub(super) async fn stop(&mut self) {
        info!("Stopping HTTP server");
        let handle = self.handle.take().unwrap();
        handle.abort();
        let _ = handle.await;
    }
}
