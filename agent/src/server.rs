use crate::config::Config;
use axum::{
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, get_service},
    Router,
};
use std::{io, net::SocketAddr, path::PathBuf};
use tokio::task::JoinHandle;
use tower_http::services::{ServeDir, ServeFile};
use tracing::info;

pub(super) fn run(address: SocketAddr, config: Config, frame_file: PathBuf) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!("Starting HTTP server on {}", address);

        let frame_svc = get_service(ServeFile::new(frame_file)).handle_error(handle_error);

        let stream_svc =
            get_service(ServeDir::new(config.video_directory)).handle_error(handle_error);

        let app = Router::new()
            .route("/player", get(player))
            .nest_service("/frame.jpg", frame_svc.clone())
            .nest_service("/", stream_svc.clone());

        axum::Server::bind(&address)
            .serve(app.into_make_service())
            .await
            .unwrap();
    })
}

async fn player() -> Html<&'static str> {
    Html(include_str!("player.html"))
}

async fn handle_error(_err: io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}
