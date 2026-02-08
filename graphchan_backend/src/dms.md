# dms.rs

## Purpose
Direct messaging service with end-to-end encryption using X25519 key exchange. Handles sending, receiving, and storing encrypted private messages between peers.

## Components

### `DmService`
- **Does**: Encrypted DM send/receive operations
- **Fields**: `database`, `paths` (for loading keys)
- **Crypto**: X25519 ECDH + ChaCha20-Poly1305

### `derive_conversation_id`
- **Does**: Deterministic conversation ID from two peer IDs
- **Algorithm**: `blake3(sort([peer_a, peer_b]).join(":"))[0:16].hex()`
- **Rationale**: Same ID regardless of who initiates, stable for lookups

### `send_dm`
- **Does**: Encrypts and stores outgoing DM, returns ciphertext for gossip broadcast
- **Returns**: `(DirectMessageView, Vec<u8>, Vec<u8>)` — (view, ciphertext, nonce)
- **Flow**:
  1. Load our X25519 secret key
  2. Get recipient's X25519 public key from database
  3. `encrypt_dm(body, our_secret, their_pubkey)`
  4. Store encrypted record with nonce
  5. Update conversation metadata
  6. Return view + raw ciphertext/nonce for gossip broadcast

### `ingest_dm`
- **Does**: Stores an encrypted DM received via gossip and attempts decryption
- **Flow**: Store raw encrypted record → call `receive_dm` for preview/conversation update
- **Interacts with**: Called from `network/ingest.rs` when gossip delivers a DirectMessage event

### `receive_dm`
- **Does**: Decrypts and stores incoming DM
- **Flow**:
  1. Load our X25519 secret key
  2. Get sender's X25519 public key
  3. `decrypt_dm(ciphertext, nonce, our_secret, their_pubkey)`
  4. Store decrypted for local viewing

### `list_conversations`
- **Does**: Lists all DM conversations with last message preview
- **Returns**: `Vec<ConversationView>` with peer info and unread counts

### `get_messages`
- **Does**: Fetches decrypted message history for a conversation
- **Interacts with**: DirectMessageRepository, decryption for each message

### `mark_read`
- **Does**: Sets read_at timestamp on a message
- **Updates**: Conversation unread_count

## Data Types

### `DirectMessageView`
- **Fields**: id, from_peer_id, to_peer_id, body (plaintext), created_at, read_at

### `ConversationView`
- **Fields**: id, peer_id, peer (PeerView), last_message, unread_count

### `DirectMessageRecord`
- **Fields**: id, conversation_id, from/to_peer_id, encrypted_body, nonce, timestamps
- **Note**: Stored encrypted, decrypted on read

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api/dms.rs` | `send_dm` returns `(view, ciphertext, nonce)` tuple | Return type |
| `network/ingest.rs` | `ingest_dm` for incoming gossip DMs | Method signature |
| `api/dms.rs` | `list_conversations`, `get_messages`, `mark_as_read`, `count_unread` | Method changes |

## Encryption Flow

```
Sender                              Recipient
  │                                     │
  │  shared = ECDH(my_secret,           │
  │               their_pubkey)         │
  │  key = HKDF(shared, "dm-key")       │
  │  nonce = random_24_bytes()          │
  │  ciphertext = ChaCha20Poly1305(     │
  │    key, nonce, plaintext)           │
  │                                     │
  │  ──── (ciphertext, nonce) ────────> │
  │                                     │
  │                   shared = ECDH(my_secret,
  │                                their_pubkey)
  │                   key = HKDF(shared, "dm-key")
  │                   plaintext = decrypt(
  │                     key, nonce, ciphertext)
```

## Notes
- X25519 keys stored in `keys/` directory
- Nonce stored per-message (24 bytes for XChaCha20)
- Forward secrecy not implemented (same shared secret per pair)
- Messages stored encrypted, decrypted on each read
- Conversation ID deterministic for deduplication
- **Missing X25519 key handling**: Peers added via short friendcode lack X25519 keys.
  `get_messages` returns empty list gracefully; `send_dm` returns descriptive error.
