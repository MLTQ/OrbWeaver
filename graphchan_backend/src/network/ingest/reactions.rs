use crate::database::models::ReactionRecord;
use crate::database::repositories::ReactionRepository;
use crate::database::Database;
use crate::network::events::ReactionUpdate;
use anyhow::Result;

pub(super) fn apply_reaction_update(database: &Database, reaction: ReactionUpdate) -> Result<()> {
    database.with_repositories(|repos| {
        if reaction.is_removal {
            tracing::info!(
                post_id = %reaction.post_id,
                reactor = %reaction.reactor_peer_id,
                emoji = %reaction.emoji,
                "👎 removing reaction via gossip"
            );
            repos.reactions().remove(
                &reaction.post_id,
                &reaction.reactor_peer_id,
                &reaction.emoji,
            )?;
        } else {
            tracing::info!(
                post_id = %reaction.post_id,
                reactor = %reaction.reactor_peer_id,
                emoji = %reaction.emoji,
                "👍 adding reaction via gossip"
            );
            let record = ReactionRecord {
                post_id: reaction.post_id,
                reactor_peer_id: reaction.reactor_peer_id,
                emoji: reaction.emoji,
                signature: reaction.signature,
                created_at: reaction.created_at,
            };
            repos.reactions().add(&record)?;
        }
        Ok(())
    })
}
