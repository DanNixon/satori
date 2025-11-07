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
use std::path::PathBuf;
use tracing::{debug, info, warn};

#[tracing::instrument(skip_all)]
pub(super) async fn handle_event_upload(
    State(state): State<AppState>,
    Json(event): Json<Event>,
) -> impl IntoResponse {
    info!("Saving event");

    let result = match state.storage.put_event(&event).await {
        Ok(_) => StatusCode::OK,
        Err(e) => {
            warn!("Failed to store event with error {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        }
    };

    crate::o11y::inc_endpoints_metric("event_upload", result);
    result
}

#[tracing::instrument(skip_all)]
pub(super) async fn handle_camera_segment_upload(
    State(state): State<AppState>,
    Path(camera): Path<String>,
    Json(cmd): Json<ArchiveSegmentCommand>,
) -> impl IntoResponse {
    info!("Saving segment");

    let result = match cmd
        .segment_url
        .path_segments()
        .and_then(|mut segments| segments.next_back())
    {
        None => {
            warn!("Malformed segment URL: {}", cmd.segment_url);
            StatusCode::BAD_REQUEST
        }
        Some(filename) => {
            debug!("Segment filename: {filename}");
            let filename = PathBuf::from(filename);

            match state.get(cmd.segment_url).await {
                Err(e) => {
                    warn!("Failed to get segment for archive storage with error: {e}");
                    StatusCode::INTERNAL_SERVER_ERROR
                }
                Ok(data) => {
                    match state
                        .storage
                        .put_segment(&camera, &filename, data)
                        .await
                        .into_diagnostic()
                    {
                        Err(e) => {
                            warn!("Failed to store segment in archive with error: {e}");
                            StatusCode::INTERNAL_SERVER_ERROR
                        }
                        Ok(_) => StatusCode::OK,
                    }
                }
            }
        }
    };

    crate::o11y::inc_endpoints_metric("camera_segment_upload", result);
    result
}
