# handlers_misc.rs

## Purpose
Handles async message responses for imports, identity/peers, reactions, DMs, search, recent posts, topics, and theme. A catch-all for handler methods that don't warrant their own file.

## Components

### Import Handlers
- `handle_import_finished` - Closes importer dialog, shows banner, triggers catalog refresh (4chan path via `ImportFinished` message)
- `handle_thread_imported` - Closes dialog, switches view to new thread, triggers load (Reddit path via `ThreadImported` message)
- `handle_import_error` - Sets error on the unified `self.importer` state

### Identity/Peer Handlers
- `handle_identity_loaded` - Updates `identity_state.local_peer`
- `handle_peers_loaded` - Inserts all peers into `self.peers` HashMap
- `handle_avatar_uploaded` - Reloads identity to get new avatar ID
- `handle_profile_updated` - Reloads identity after profile change
- `handle_peer_added` - Inserts peer, shows banner, clears friendcode input
- `handle_thread_files_selected` - Appends selected files to `create_thread.files`

### Reaction Handlers
- `handle_reactions_loaded` - Stores reactions in `ThreadState`, clears loading flag
- `handle_reaction_added` / `handle_reaction_removed` - Triggers reaction reload for the post

### DM Handlers
- `handle_conversations_loaded` - Updates `dm_state.conversations`
- `handle_messages_loaded` - Updates messages for active conversation (matched by `peer_id`)
- `handle_dm_sent` - Appends sent message, clears input, reloads conversations for unread counts

### Search Handler
- `handle_search_completed` - Updates `SearchState` results (matched by query string)

### Recent Posts Handler
- `handle_recent_posts_loaded` - Updates `recent_posts` feed

### Topic Handlers
- `handle_topics_loaded` - Updates `subscribed_topics`
- `handle_topic_subscribed` - Appends topic, clears input
- `handle_topic_unsubscribed` - Removes topic from both `subscribed_topics` and `selected_topics`

### Theme Handler
- `handle_theme_color_loaded` - Sets `primary_color` from RGB, marks `theme_dirty` for reapply

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `messages.rs` | All handler methods exist on `GraphchanApp` | Method removal/rename |
| `spawners.rs` | Handlers call `spawn_load_identity`, `spawn_load_conversations`, etc. | Spawner removal |
| `state.rs` | `DmState`, `IdentityState`, `ImporterState`, etc. field shapes | Field changes |

## Notes
- `handle_dm_sent` reloads the full conversation list to update unread counts
- `handle_import_error` sets error directly on unified `self.importer` (previously dispatched to separate 4chan/Reddit importers)
- `handle_theme_color_loaded` falls back to default blue `(64, 128, 255)` on error
- Reaction handlers always reload rather than optimistically updating, ensuring consistency
