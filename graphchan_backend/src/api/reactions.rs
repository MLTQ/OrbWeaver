use super::{AppState, ApiError, ApiResult};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct AddReactionRequest {
    emoji: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RemoveReactionRequest {
    emoji: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReactionsResponse {
    reactions: Vec<ReactionView>,
    counts: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ReactionView {
    emoji: String,
    reactor_peer_id: String,
    created_at: String,
}

pub(crate) async fn get_post_reactions(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
) -> ApiResult<ReactionsResponse> {
    use crate::database::repositories::ReactionRepository;

    let (reactions, counts) = state.database.with_repositories(|repos| {
        let reactions = repos.reactions().list_for_post(&post_id)?;
        let counts = repos.reactions().count_for_post(&post_id)?;
        Ok((reactions, counts))
    })?;

    let reaction_views: Vec<ReactionView> = reactions
        .into_iter()
        .map(|r| ReactionView {
            emoji: r.emoji,
            reactor_peer_id: r.reactor_peer_id,
            created_at: r.created_at,
        })
        .collect();

    Ok(Json(ReactionsResponse {
        reactions: reaction_views,
        counts,
    }))
}

pub(crate) async fn add_reaction(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
    Json(payload): Json<AddReactionRequest>,
) -> Result<StatusCode, ApiError> {
    use crate::database::repositories::ReactionRepository;
    use crate::database::models::ReactionRecord;

    // Get local identity to sign the reaction
    let identity: String = state.database.with_repositories(|repos| {
        let result: Result<String, rusqlite::Error> = repos.conn().query_row(
            "SELECT gpg_fingerprint FROM node_identity WHERE id = 1",
            [],
            |row| row.get(0),
        );
        result.map_err(|e| anyhow::Error::from(e))
    })?;

    // Create signature (simplified - in production would use GPG)
    let signature = format!("sig:{}:{}:{}", post_id, identity, payload.emoji);
    let created_at = chrono::Utc::now().to_rfc3339();

    let reaction = ReactionRecord {
        post_id: post_id.clone(),
        reactor_peer_id: identity,
        emoji: payload.emoji,
        signature,
        created_at,
    };

    state.database.with_repositories(|repos| {
        repos.reactions().add(&reaction)
    })?;

    // Get thread_id for the post
    let thread_id: String = state.database.with_repositories(|repos| {
        use crate::database::repositories::PostRepository;
        let post = repos.posts().get(&post_id)?;
        Ok(post.ok_or_else(|| anyhow::anyhow!("Post not found"))?.thread_id)
    })?;

    // Broadcast via gossip
    let reaction_update = crate::network::ReactionUpdate {
        post_id,
        thread_id,
        reactor_peer_id: reaction.reactor_peer_id.clone(),
        emoji: reaction.emoji.clone(),
        signature: reaction.signature.clone(),
        created_at: reaction.created_at.clone(),
        is_removal: false,
    };

    state.network.publish_reaction_update(reaction_update).await?;

    Ok(StatusCode::OK)
}

pub(crate) async fn remove_reaction(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
    Json(payload): Json<RemoveReactionRequest>,
) -> Result<StatusCode, ApiError> {
    use crate::database::repositories::ReactionRepository;

    let identity: String = state.database.with_repositories(|repos| {
        let result: Result<String, rusqlite::Error> = repos.conn().query_row(
            "SELECT gpg_fingerprint FROM node_identity WHERE id = 1",
            [],
            |row| row.get(0),
        );
        result.map_err(|e| anyhow::Error::from(e))
    })?;

    state.database.with_repositories(|repos| {
        repos.reactions().remove(&post_id, &identity, &payload.emoji)
    })?;

    // Get thread_id for the post
    let thread_id: String = state.database.with_repositories(|repos| {
        use crate::database::repositories::PostRepository;
        let post = repos.posts().get(&post_id)?;
        Ok(post.ok_or_else(|| anyhow::anyhow!("Post not found"))?.thread_id)
    })?;

    // Broadcast unreact via gossip
    let reaction_update = crate::network::ReactionUpdate {
        post_id,
        thread_id,
        reactor_peer_id: identity,
        emoji: payload.emoji,
        signature: "".to_string(), // Not needed for removal
        created_at: chrono::Utc::now().to_rfc3339(),
        is_removal: true,
    };

    state.network.publish_reaction_update(reaction_update).await?;

    Ok(StatusCode::OK)
}
