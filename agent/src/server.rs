use crate::config::Config;
use axum::{response::Html, routing::get, Router};
use std::{net::SocketAddr, path::PathBuf};
use tokio::task::JoinHandle;
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

pub(super) fn run(address: SocketAddr, config: Config, frame_file: PathBuf) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("Starting HTTP server on {}", address);

        let frame_svc = ServeFile::new(frame_file);

        let stream_svc = ServeDir::new(config.video_directory);

        let player = { Html(include_str!("player.html")) };

        let app = Router::new()
            .route("/player", get(player))
            .nest_service("/frame.jpg", frame_svc)
            .nest_service("/", stream_svc);

        let listener = tokio::net::TcpListener::bind(&address)
            .await
            .unwrap_or_else(|_| panic!("tcp listener should bind to {address}"));

        axum::serve(listener, app).await.unwrap();
    })
}
