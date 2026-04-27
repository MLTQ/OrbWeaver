use super::{ApiError, ApiResult, AppState};
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
    if payload.body.trim().is_empty() {
        return Err(ApiError::BadRequest("message body may not be empty".into()));
    }

    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let (message, ciphertext, nonce) = service
        .send_dm(&payload.to_peer_id, &payload.body)
        .map_err(map_send_dm_error)?;

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

#[derive(Debug, Serialize)]
pub(crate) struct MarkConversationReadResponse {
    /// Number of incoming messages newly transitioned from unread → read.
    /// 0 when the conversation was already fully read.
    marked: usize,
}

pub(crate) async fn mark_conversation_read_handler(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> ApiResult<MarkConversationReadResponse> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let marked = service
        .mark_conversation_read(&peer_id)
        .map_err(ApiError::Internal)?;
    Ok(Json(MarkConversationReadResponse { marked }))
}

pub(crate) async fn count_unread_handler(
    State(state): State<AppState>,
) -> ApiResult<UnreadCountResponse> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let count = service.count_unread().map_err(ApiError::Internal)?;
    Ok(Json(UnreadCountResponse { count }))
}

/// DmService::send_dm bubbles every failure through anyhow::Error, but several
/// of those are user-fixable (peer unknown, peer has no x25519 key, malformed
/// pubkey, etc.). Without this mapping they all become 500 "internal server
/// error" with no body, which is unactionable for clients. We pattern-match the
/// known user-facing error strings and surface them as 400/404 with the
/// original message preserved.
fn map_send_dm_error(err: anyhow::Error) -> ApiError {
    let msg = err.to_string();
    if msg.contains("peer not found") {
        ApiError::NotFound(msg)
    } else if msg.contains("no X25519 public key")
        || msg.contains("invalid X25519 public key length")
        || msg.contains("failed to decode X25519 public key")
    {
        ApiError::BadRequest(msg)
    } else {
        ApiError::Internal(err)
    }
}
