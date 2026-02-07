use super::{AppState, ApiError, ApiResult};
use crate::database::repositories::PeerRepository;
use crate::files::FileService;
use crate::identity::decode_friendcode_auto;
use crate::network::ProfileUpdate;
use crate::peers::{PeerService, PeerView};
use axum::extract::{Multipart, Path, State};
use axum::http::StatusCode;
use axum::Json;
use iroh_blobs::ticket::BlobTicket;
use iroh_blobs::{BlobFormat, Hash};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Debug, Deserialize)]
pub(crate) struct AddPeerRequest {
    friendcode: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UpdateProfileRequest {
    pub username: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddAgentRequest {
    pub name: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct AgentsResponse {
    pub agents: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ThemeColorResponse {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetThemeColorRequest {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub(crate) async fn list_peers(State(state): State<AppState>) -> ApiResult<Vec<PeerView>> {
    let service = PeerService::new(state.database.clone());
    let peers = service.list_peers()?;
    Ok(Json(peers))
}

pub(crate) async fn get_self_peer(State(state): State<AppState>) -> ApiResult<Option<PeerView>> {
    let service = PeerService::new(state.database.clone());
    let mut peer = match service.get_local_peer()? {
        Some(p) => p,
        None => return Ok(Json(None)),
    };

    // Generate a full friend code with network addresses (including relay URL)
    // This ensures the friend code can be used for NAT traversal
    if let (Some(iroh_peer_id), Some(gpg_fingerprint)) = (&peer.iroh_peer_id, &peer.gpg_fingerprint) {
        let addresses = state.network.get_addresses();
        if !addresses.is_empty() {
            // Create a friend code payload with addresses
            let payload = crate::identity::FriendCodePayload {
                version: 2,
                peer_id: iroh_peer_id.clone(),
                gpg_fingerprint: gpg_fingerprint.clone(),
                x25519_pubkey: peer.x25519_pubkey.clone(),
                addresses,
            };
            if let Ok(json) = serde_json::to_vec(&payload) {
                use base64::Engine;
                peer.friendcode = Some(base64::engine::general_purpose::STANDARD.encode(json));
            }
        }
    }

    Ok(Json(Some(peer)))
}

pub(crate) async fn add_peer(
    State(state): State<AppState>,
    Json(request): Json<AddPeerRequest>,
) -> Result<(StatusCode, Json<PeerView>), ApiError> {
    let service = PeerService::new(state.database.clone());
    let friendcode = request.friendcode.trim();
    match service.register_friendcode(friendcode) {
        Ok(peer) => {
            // Connect to the peer and get their iroh peer ID
            let iroh_peer_id = if let Ok(payload) = decode_friendcode_auto(friendcode) {
                match state.network.connect_friendcode(&payload).await {
                    Ok(peer_id) => Some(peer_id),
                    Err(err) => {
                        tracing::warn!(error = ?err, "failed to connect to peer after registering friendcode");
                        None
                    }
                }
            } else {
                None
            };

            // Subscribe to this peer's topic to receive their announcements
            // Use the iroh peer ID as bootstrap to help establish gossip connectivity
            if let Err(err) = state.network.subscribe_to_peer(&peer.id, iroh_peer_id).await {
                tracing::warn!(error = ?err, peer_id = %peer.id, "failed to subscribe to peer topic");
            }

            Ok((StatusCode::CREATED, Json(peer)))
        }
        Err(err) if err.to_string().contains("decode friendcode") => {
            Err(ApiError::BadRequest("invalid friendcode".into()))
        }
        Err(err) => Err(ApiError::Internal(err)),
    }
}

pub(crate) async fn unfollow_peer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.database.with_repositories(|repos| {
        repos.peers().delete(&id)
    })?;
    Ok(StatusCode::OK)
}

pub(crate) async fn upload_avatar(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<String>), ApiError> {
    let service = FileService::new(
        state.database.clone(),
        state.config.paths.clone(),
        state.config.file.clone(),
        state.blobs.clone(),
    );
    let mut file_bytes = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field
                .bytes()
                .await
                .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?;
            file_bytes = Some(data.to_vec());
            tracing::info!("Avatar file received, size: {}", data.len());
            break;
        } else {
            tracing::debug!("Ignored field in avatar upload: {}", name);
        }
    }

    let Some(bytes) = file_bytes else {
        return Err(ApiError::BadRequest("missing file field".into()));
    };

    let blob_id = service.import_blob(bytes).await.map_err(ApiError::Internal)?;

    // Update local peer profile
    let peer_service = PeerService::new(state.database.clone());
    // We need the local peer ID (fingerprint).
    // We can get it from state.identity.gpg_fingerprint.
    let peer_id = state.identity.gpg_fingerprint.clone();
    peer_service.update_profile(&peer_id, Some(blob_id.clone()), None, None, None).map_err(ApiError::Internal)?;

    // Generate ticket
    let hash = Hash::from_str(&blob_id).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    let addr = state.network.current_addr();
    let ticket = BlobTicket::new(addr, hash, BlobFormat::Raw);

    // Broadcast ProfileUpdate
    let update = ProfileUpdate {
        peer_id: peer_id.clone(),
        avatar_file_id: Some(blob_id.clone()),
        ticket: Some(ticket),
        username: None,
        bio: None,
        agents: None,
    };
    state.network.publish_profile_update(update).await.map_err(ApiError::Internal)?;

    Ok((StatusCode::OK, Json(blob_id)))
}

pub(crate) async fn update_profile_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<StatusCode, ApiError> {
    let peer_service = PeerService::new(state.database.clone());
    let peer_id = state.identity.gpg_fingerprint.clone();

    peer_service.update_profile(&peer_id, None, payload.username.clone(), payload.bio.clone(), None)
        .map_err(ApiError::Internal)?;

    // Broadcast ProfileUpdate
    // We need to fetch the current avatar ticket if we want to include it,
    // or we can make the fields optional in ProfileUpdate too.
    // Let's assume ProfileUpdate needs to be updated to support optional fields.
    // For now, let's just send what we have.

    // We need to get the current avatar ticket to send it along, or send None if we don't want to change it.
    // But broadcast_profile_update replaces the state usually.
    // Let's check ProfileUpdate struct in network.rs.

    let update = ProfileUpdate {
        peer_id: peer_id.clone(),
        avatar_file_id: None,
        ticket: None,
        username: payload.username,
        bio: payload.bio,
        agents: None,
    };
    state.network.publish_profile_update(update).await.map_err(ApiError::Internal)?;

    Ok(StatusCode::OK)
}

pub(crate) async fn get_agents_handler(
    State(state): State<AppState>,
) -> Result<Json<AgentsResponse>, ApiError> {
    let peer_service = PeerService::new(state.database.clone());

    let peer = peer_service.get_local_peer().map_err(ApiError::Internal)?;
    let agents = peer.and_then(|p| p.agents).unwrap_or_default();

    Ok(Json(AgentsResponse { agents }))
}

pub(crate) async fn add_agent_handler(
    State(state): State<AppState>,
    Json(payload): Json<AddAgentRequest>,
) -> Result<StatusCode, ApiError> {
    let peer_id = state.identity.gpg_fingerprint.clone();
    let peer_service = PeerService::new(state.database.clone());

    // Get current agents list
    let peer = peer_service.get_local_peer().map_err(ApiError::Internal)?;
    let mut agents = peer.and_then(|p| p.agents).unwrap_or_default();

    // Add new agent if not already present
    if !agents.contains(&payload.name) {
        agents.push(payload.name.clone());

        // Update profile with new agents list
        peer_service.update_profile(&peer_id, None, None, None, Some(agents.clone()))
            .map_err(ApiError::Internal)?;

        // Broadcast ProfileUpdate
        let update = ProfileUpdate {
            peer_id: peer_id.clone(),
            avatar_file_id: None,
            ticket: None,
            username: None,
            bio: None,
            agents: Some(agents),
        };
        state.network.publish_profile_update(update).await.map_err(ApiError::Internal)?;
    }

    Ok(StatusCode::OK)
}

pub(crate) async fn remove_agent_handler(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    let peer_id = state.identity.gpg_fingerprint.clone();
    let peer_service = PeerService::new(state.database.clone());

    // Get current agents list
    let peer = peer_service.get_local_peer().map_err(ApiError::Internal)?;
    let mut agents = peer.and_then(|p| p.agents).unwrap_or_default();

    // Remove agent if present
    if agents.iter().position(|a| a == &name).is_some() {
        agents.retain(|a| a != &name);

        // Update profile with new agents list
        peer_service.update_profile(&peer_id, None, None, None, Some(agents.clone()))
            .map_err(ApiError::Internal)?;

        // Broadcast ProfileUpdate
        let update = ProfileUpdate {
            peer_id: peer_id.clone(),
            avatar_file_id: None,
            ticket: None,
            username: None,
            bio: None,
            agents: Some(agents),
        };
        state.network.publish_profile_update(update).await.map_err(ApiError::Internal)?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn get_theme_color_handler(
    State(state): State<AppState>,
) -> Result<Json<ThemeColorResponse>, ApiError> {
    // Default to a nice blue if not set
    let default_color = ThemeColorResponse { r: 64, g: 128, b: 255 };

    let color_str = state.database.get_setting("theme_color")
        .map_err(ApiError::Internal)?;

    if let Some(color_str) = color_str {
        // Parse format: "r,g,b"
        let parts: Vec<&str> = color_str.split(',').collect();
        if parts.len() == 3 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].parse::<u8>(),
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
            ) {
                return Ok(Json(ThemeColorResponse { r, g, b }));
            }
        }
    }

    Ok(Json(default_color))
}

pub(crate) async fn set_theme_color_handler(
    State(state): State<AppState>,
    Json(payload): Json<SetThemeColorRequest>,
) -> Result<StatusCode, ApiError> {
    let color_str = format!("{},{},{}", payload.r, payload.g, payload.b);
    state.database.set_setting("theme_color", &color_str)
        .map_err(ApiError::Internal)?;

    Ok(StatusCode::OK)
}
