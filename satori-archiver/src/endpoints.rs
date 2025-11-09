use crate::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use miette::IntoDiagnostic;
use satori_common::{ArchiveSegmentCommand, Event};
use satori_storage::StorageProvider;
use std::{path::PathBuf, sync::Arc};
use tracing::{info, warn};

#[tracing::instrument(skip_all)]
pub(super) async fn handle_event_upload(
    State(state): State<Arc<AppState>>,
    Json(event): Json<Event>,
) -> impl IntoResponse {
    info!("Saving event");

    match state.storage.put_event(&event).await {
        Ok(_) => {
            // TODO: metrics
            (StatusCode::OK, String::new())
        }
        Err(e) => {
            warn!("Failed to store event with error {e}");
            // TODO: metrics
            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
        }
    }
}

#[tracing::instrument(skip_all)]
pub(super) async fn handle_camera_segment_upload(
    State(state): State<Arc<AppState>>,
    Path(camera): Path<String>,
    Json(cmd): Json<ArchiveSegmentCommand>,
) -> impl IntoResponse {
    info!("Saving segment");

    match cmd
        .segment_url
        .path_segments()
        .and_then(|segments| segments.last())
    {
        None => {
            // TODO: metrics
            (StatusCode::BAD_REQUEST, String::new())
        }
        Some(filename) => {
            let filename = PathBuf::from(filename);

            match state.get(cmd.segment_url).await {
                Err(_) => {
                    // TODO: metrics
                    (StatusCode::INTERNAL_SERVER_ERROR, String::new())
                }
                Ok(data) => {
                    match state
                        .storage
                        .put_segment(&camera, &filename, data)
                        .await
                        .into_diagnostic()
                    {
                        Err(_) => {
                            // TODO: metrics
                            (StatusCode::INTERNAL_SERVER_ERROR, String::new())
                        }
                        Ok(_) => {
                            // TODO: metrics
                            (StatusCode::OK, String::new())
                        }
                    }
                }
            }
        }
    }
}
