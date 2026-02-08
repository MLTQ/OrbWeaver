use super::{AppState, ApiError, ApiResult};
use crate::dms::{ConversationView, DirectMessageView, DmService};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct SendDmRequest {
    to_peer_id: String,
    body: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GetMessagesParams {
    #[serde(default = "default_limit")]
    limit: usize,
}

pub(crate) fn default_limit() -> usize {
    50
}

#[derive(Debug, Serialize)]
pub(crate) struct UnreadCountResponse {
    count: usize,
}

pub(crate) async fn list_conversations_handler(
    State(state): State<AppState>,
) -> ApiResult<Vec<ConversationView>> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let conversations = service.list_conversations().map_err(ApiError::Internal)?;
    Ok(Json(conversations))
}

pub(crate) async fn send_dm_handler(
    State(state): State<AppState>,
    Json(payload): Json<SendDmRequest>,
) -> Result<(StatusCode, Json<DirectMessageView>), ApiError> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let (message, ciphertext, nonce) = service
        .send_dm(&payload.to_peer_id, &payload.body)
        .map_err(ApiError::Internal)?;

    // Broadcast encrypted DM over gossip to recipient
    let dm_event = crate::network::DirectMessageEvent {
        from_peer_id: message.from_peer_id.clone(),
        to_peer_id: message.to_peer_id.clone(),
        encrypted_body: ciphertext,
        nonce,
        message_id: message.id.clone(),
        conversation_id: message.conversation_id.clone(),
        created_at: message.created_at.clone(),
    };
    if let Err(err) = state.network.publish_direct_message(dm_event).await {
        tracing::warn!(error = ?err, "failed to broadcast DM over gossip");
    }

    Ok((StatusCode::CREATED, Json(message)))
}

pub(crate) async fn get_messages_handler(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
    Query(params): Query<GetMessagesParams>,
) -> ApiResult<Vec<DirectMessageView>> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let limit = params.limit.min(200);
    let messages = service
        .get_messages(&peer_id, limit)
        .map_err(ApiError::Internal)?;
    Ok(Json(messages))
}

pub(crate) async fn mark_message_read_handler(
    State(state): State<AppState>,
    Path(message_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    service
        .mark_as_read(&message_id)
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::OK)
}

pub(crate) async fn count_unread_handler(
    State(state): State<AppState>,
) -> ApiResult<UnreadCountResponse> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let count = service.count_unread().map_err(ApiError::Internal)?;
    Ok(Json(UnreadCountResponse { count }))
}
