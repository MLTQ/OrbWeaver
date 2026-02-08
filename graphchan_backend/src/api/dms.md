# dms.rs (API handlers)

## Purpose
Axum HTTP handlers for direct messaging endpoints. Bridges REST API requests to `DmService` and broadcasts DM events over gossip.

## Components

### `send_dm_handler`
- **Does**: Encrypts and sends a DM, broadcasts ciphertext over gossip
- **Route**: `POST /dms/send`
- **Flow**: Call `DmService::send_dm` → construct `DirectMessageEvent` → publish via gossip
- **Interacts with**: `DmService`, `NetworkHandle::publish_direct_message`

### `list_conversations_handler`
- **Does**: Returns all DM conversations with peer info and unread counts
- **Route**: `GET /dms/conversations`

### `get_messages_handler`
- **Does**: Returns decrypted message history for a peer conversation
- **Route**: `GET /dms/{peer_id}/messages`
- **Params**: `limit` query param (default 50, max 200)

### `mark_message_read_handler`
- **Does**: Marks a message as read
- **Route**: `POST /dms/messages/{message_id}/read`

### `count_unread_handler`
- **Does**: Returns total unread DM count
- **Route**: `GET /dms/unread/count`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api/mod.rs` | Handler function signatures match Axum router | Signature changes |
| `DmService` | `send_dm` returns `(view, ciphertext, nonce)` | Return type |
| `NetworkHandle` | `publish_direct_message` accepts `DirectMessageEvent` | Method signature |
