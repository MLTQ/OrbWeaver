use crate::database::Database;
use crate::network::events::ProfileUpdate;
use crate::peers::PeerService;
use anyhow::Result;

pub(super) fn apply_profile_update(database: &Database, update: ProfileUpdate) -> Result<()> {
    let service = PeerService::new(database.clone());
    service.update_profile(
        &update.peer_id,
        update.avatar_file_id,
        update.username,
        update.bio,
        update.agents,
        update.x25519_pubkey,
    )?;
    Ok(())
}
