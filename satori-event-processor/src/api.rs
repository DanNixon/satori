use crate::AppState;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use satori_common::TriggerCommand;
use tokio::time::Instant;
use tracing::debug;

#[tracing::instrument(skip_all)]
pub(super) async fn handle_trigger(
    State(state): State<AppState>,
    Json(cmd): Json<TriggerCommand>,
) -> impl IntoResponse {
    debug!("Trigger command: {:?}", cmd);

    // Derive the full trigger
    let trigger = state.trigger_config.create_trigger(&cmd);

    // Evaluate trigger
    {
        let mut events = state.events.lock().await;
        events.trigger(&trigger).await;
    }

    // Signal that event processing should happen right away
    let _ = state.process_trigger.send_replace(Instant::now());

    StatusCode::OK
}
