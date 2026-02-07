use super::{AppState, ApiError, ApiResult};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct SetSettingRequest {
    value: String,
}

#[derive(Deserialize)]
pub(crate) struct SubscribeTopicRequest {
    topic_id: String,
}

pub(crate) async fn get_setting_handler(
    State(state): State<AppState>,
    Path(key): Path<String>,
) -> ApiResult<Option<String>> {
    let value = state.database
        .get_setting(&key)
        .map_err(ApiError::Internal)?;
    Ok(Json(value))
}

pub(crate) async fn set_setting_handler(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(req): Json<SetSettingRequest>,
) -> Result<StatusCode, ApiError> {
    state.database
        .set_setting(&key, &req.value)
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn list_topics_handler(
    State(state): State<AppState>,
) -> ApiResult<Vec<String>> {
    use crate::database::repositories::TopicRepository;

    let topics = state.database.with_repositories(|repos| {
        repos.topics().list_subscribed()
    }).map_err(ApiError::Internal)?;

    Ok(Json(topics))
}

pub(crate) async fn subscribe_topic_handler(
    State(state): State<AppState>,
    Json(req): Json<SubscribeTopicRequest>,
) -> Result<StatusCode, ApiError> {
    use crate::database::repositories::TopicRepository;

    // Subscribe in database
    state.database.with_repositories(|repos| {
        repos.topics().subscribe(&req.topic_id)
    }).map_err(ApiError::Internal)?;

    // Subscribe to the gossip topic
    state.network.subscribe_to_topic(&req.topic_id).await
        .map_err(ApiError::Internal)?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn unsubscribe_topic_handler(
    State(state): State<AppState>,
    Path(topic_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    use crate::database::repositories::TopicRepository;

    state.database.with_repositories(|repos| {
        repos.topics().unsubscribe(&topic_id)
    }).map_err(ApiError::Internal)?;

    // Note: We don't unsubscribe from the gossip topic because it's harmless to stay subscribed
    // and might cause issues if we re-subscribe later

    Ok(StatusCode::NO_CONTENT)
}
