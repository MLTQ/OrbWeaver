# OrbWeaver Privacy & Moderation Implementation Plan

## Overview

This document outlines the implementation plan for:
1. **X25519 Encryption Identity** — Separate keypair for encryption (alongside GPG for signing)
2. **Thread Visibility Model** — Social and Private threads
3. **Direct Messages (DMs)** — Persistent, encrypted 1:1 conversations
4. **Blocking & Blocklists** — Client-side content filtering with DAG preservation

## Design Principles

- **Blocking is client-side** — Your node, your rules
- **DAG is sacred** — Always preserve thread structure with placeholders
- **Content is fetch-on-demand** — Missing content can be filled from other sources
- **Trust is portable** — Shared blocklists enable inherited curation
- **Social propagation** — "Public" means friends-of-friends-of-friends, not the entire internet

---

## Part 1: X25519 Encryption Identity

### Motivation

Currently we have GPG keys for signing and identity verification. We need a separate X25519 keypair for Diffie-Hellman key exchange and encryption operations.

### New Dependencies

```toml
# graphchan_backend/Cargo.toml
x25519-dalek = "2.0"
crypto_box = "0.9"
chacha20poly1305 = "0.10"
hkdf = "0.12"
```

### New Files

```
graphchan_backend/src/
├── crypto/
│   ├── mod.rs           # Module exports
│   ├── keys.rs          # X25519 keypair management
│   ├── thread_crypto.rs # Thread key derivation/wrapping
│   ├── dm_crypto.rs     # DM encryption (crypto_box)
│   └── utils.rs         # HKDF, nonce generation, etc.
```

### Database Schema Changes

```sql
-- Add X25519 public key to peers
ALTER TABLE peers ADD COLUMN x25519_pubkey BLOB;

-- Local encryption keypair (stored in keys/ directory, but reference here)
-- The actual secret key is in keys/encryption.key (similar to iroh.key)
```

### Key Storage

```
keys/
├── iroh.key           # Existing Iroh secret key
├── gpg/               # Existing GPG keyring
│   ├── private.asc
│   └── public.asc
└── encryption.key     # NEW: X25519 secret key (JSON format)
```

### Friendcode Extension

Current friendcode contains: `{ peer_id, gpg_fingerprint, addresses[] }`

Extended: `{ peer_id, gpg_fingerprint, x25519_pubkey, addresses[] }`

This is a breaking change to friendcode format — bump version to `2`.

### Implementation Tasks

1. [ ] Create `crypto` module structure
2. [ ] Implement X25519 keypair generation and persistence
3. [ ] Extend `IdentitySummary` to include X25519 public key
4. [ ] Update friendcode encoding/decoding (version 2)
5. [ ] Add `x25519_pubkey` column to peers table
6. [ ] Update peer registration to store X25519 pubkey
7. [ ] Add migration for existing peers (nullable initially)

---

## Part 2: Thread Visibility Model

### Visibility Levels

```rust
pub enum ThreadVisibility {
    /// Secret topic hash(thread_id + topic_secret)
    /// Propagates through social sharing only
    /// Use case: Normal conversations, default mode
    Social,
    
    /// Secret topic + encrypted content
    /// Explicit member list, wrapped keys
    /// Use case: Private group chats, sensitive discussions
    Private,
}
```

### Topic Derivation

```rust
/// Social threads: requires secret to derive
fn social_thread_topic(thread_id: &str, topic_secret: &[u8; 32]) -> TopicId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"orbweaver-social-v1:");
    hasher.update(thread_id.as_bytes());
    hasher.update(b":");
    hasher.update(topic_secret);
    TopicId::from_bytes(*hasher.finalize().as_bytes())
}

/// Private threads: same as social (secret topic)
/// but content is also encrypted
fn private_thread_topic(thread_id: &str, topic_secret: &[u8; 32]) -> TopicId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"orbweaver-private-v1:");
    hasher.update(thread_id.as_bytes());
    hasher.update(b":");
    hasher.update(topic_secret);
    TopicId::from_bytes(*hasher.finalize().as_bytes())
}
```

### Database Schema Changes

```sql
-- Thread visibility and secrets
ALTER TABLE threads ADD COLUMN visibility TEXT DEFAULT 'social';
ALTER TABLE threads ADD COLUMN topic_secret BLOB NOT NULL;  -- 32 bytes, always required

-- Private thread member keys
CREATE TABLE thread_member_keys (
    thread_id TEXT NOT NULL,
    peer_id TEXT NOT NULL,
    wrapped_key BLOB NOT NULL,      -- Thread key encrypted to member's X25519 pubkey
    nonce BLOB NOT NULL,            -- 24 bytes for crypto_box
    added_by_peer_id TEXT NOT NULL, -- Who added this member
    created_at TEXT NOT NULL,
    PRIMARY KEY (thread_id, peer_id),
    FOREIGN KEY (thread_id) REFERENCES threads(id)
);
```

### Thread Announcement Changes

```rust
pub struct ThreadAnnouncement {
    pub thread_id: String,
    pub visibility: ThreadVisibility,
    
    // Plaintext metadata (visible to anyone with topic_secret)
    pub title: Option<String>,
    pub preview: Option<String>,
    pub post_count: u32,
    pub has_images: bool,
    
    // For Social/Private: included so recipient can subscribe
    pub topic_secret: Option<[u8; 32]>,
    
    // For Private: wrapped thread key for the specific recipient
    // (announcements are sent per-recipient for private threads)
    pub wrapped_thread_key: Option<WrappedKey>,
    
    pub ticket: BlobTicket,
    pub thread_hash: String,
    pub created_at: String,
    pub creator_peer_id: String,
}

pub struct WrappedKey {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 24],
}
```

### Content Encryption (Private Threads Only)

```rust
/// Encrypt thread blob for private threads
fn encrypt_thread_blob(
    thread_details: &ThreadDetails,
    thread_key: &[u8; 32],
) -> Result<Vec<u8>> {
    let plaintext = serde_json::to_vec(thread_details)?;
    let cipher = ChaCha20Poly1305::new(thread_key.into());
    let nonce = ChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = cipher.encrypt(&nonce, plaintext.as_ref())?;
    
    // Prepend nonce to ciphertext
    let mut result = nonce.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypt thread blob
fn decrypt_thread_blob(
    encrypted: &[u8],
    thread_key: &[u8; 32],
) -> Result<ThreadDetails> {
    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = ChaCha20Poly1305::new(thread_key.into());
    let plaintext = cipher.decrypt(nonce, ciphertext)?;
    Ok(serde_json::from_slice(&plaintext)?)
}

/// Derive file encryption key from thread key
fn derive_file_key(thread_key: &[u8; 32], file_id: &str) -> [u8; 32] {
    let hk = Hkdf::<Sha256>::new(None, thread_key);
    let mut file_key = [0u8; 32];
    hk.expand(
        format!("orbweaver-file-v1:{}", file_id).as_bytes(),
        &mut file_key
    ).expect("valid length");
    file_key
}
```

### Implementation Tasks

1. [ ] Add `visibility` and `topic_secret` columns to threads table
2. [ ] Create `thread_member_keys` table
3. [ ] Implement topic derivation functions for each visibility level
4. [ ] Update `ThreadService::create_thread` to handle visibility
5. [ ] Implement thread key generation for private threads
6. [ ] Implement thread key wrapping (crypto_box to each member)
7. [ ] Update `ThreadAnnouncement` struct and serialization
8. [ ] Update `publish_thread_announcement` to use correct topic
9. [ ] Update thread subscription to derive topic correctly
10. [ ] Implement blob encryption/decryption for private threads
11. [ ] Implement file encryption for private thread attachments
12. [ ] Update `apply_thread_announcement` to handle new fields
13. [ ] Add "invite to thread" flow for adding members to social/private threads
14. [ ] Frontend: Thread creation UI with visibility selector
15. [ ] Frontend: Display visibility indicator on threads
16. [ ] Frontend: "Share thread" flow that includes topic_secret

---

## Part 3: Direct Messages (DMs)

### Data Model

```rust
pub struct DirectMessage {
    pub id: String,
    pub conversation_id: String,  // Deterministic: sorted hash of both peer IDs
    pub from_peer_id: String,
    pub to_peer_id: String,
    pub encrypted_body: Vec<u8>,  // crypto_box ciphertext
    pub nonce: [u8; 24],
    pub created_at: String,
    pub read_at: Option<String>,  // Local-only, when recipient read it
}

pub struct Conversation {
    pub id: String,
    pub peer_id: String,          // The other party
    pub last_message_at: String,
    pub unread_count: u32,        // Local-only
}
```

### Conversation ID Derivation

```rust
fn conversation_id(peer_a: &str, peer_b: &str) -> String {
    let mut peers = [peer_a, peer_b];
    peers.sort();
    let hash = blake3::hash(format!("orbweaver-dm-v1:{}:{}", peers[0], peers[1]).as_bytes());
    hex::encode(&hash.as_bytes()[..16])  // 32 char hex string
}
```

### DM Gossip Topic

```rust
fn dm_topic(conversation_id: &str, shared_secret: &[u8; 32]) -> TopicId {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"orbweaver-dm-topic-v1:");
    hasher.update(conversation_id.as_bytes());
    hasher.update(b":");
    hasher.update(shared_secret);
    TopicId::from_bytes(*hasher.finalize().as_bytes())
}

/// Derive shared secret for DM topic from X25519 key exchange
fn dm_shared_secret(my_secret: &StaticSecret, their_pubkey: &PublicKey) -> [u8; 32] {
    let shared = my_secret.diffie_hellman(their_pubkey);
    // Use HKDF to derive a proper key
    let hk = Hkdf::<Sha256>::new(None, shared.as_bytes());
    let mut secret = [0u8; 32];
    hk.expand(b"orbweaver-dm-secret-v1", &mut secret).expect("valid length");
    secret
}
```

### Database Schema

```sql
CREATE TABLE direct_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    from_peer_id TEXT NOT NULL,
    to_peer_id TEXT NOT NULL,
    encrypted_body BLOB NOT NULL,
    nonce BLOB NOT NULL,
    created_at TEXT NOT NULL,
    read_at TEXT,  -- NULL until read
    FOREIGN KEY (from_peer_id) REFERENCES peers(id),
    FOREIGN KEY (to_peer_id) REFERENCES peers(id)
);

CREATE INDEX idx_dm_conversation ON direct_messages(conversation_id, created_at);
CREATE INDEX idx_dm_unread ON direct_messages(to_peer_id, read_at) WHERE read_at IS NULL;

-- Conversation metadata (local cache)
CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    peer_id TEXT NOT NULL,
    last_message_at TEXT,
    last_message_preview TEXT,  -- Decrypted locally for display
    unread_count INTEGER DEFAULT 0,
    FOREIGN KEY (peer_id) REFERENCES peers(id)
);
```

### Encryption Flow

```rust
/// Encrypt a DM using crypto_box
fn encrypt_dm(
    body: &str,
    my_secret: &StaticSecret,
    their_pubkey: &PublicKey,
) -> Result<(Vec<u8>, [u8; 24])> {
    let my_pubkey = PublicKey::from(my_secret);
    let salsa_box = crypto_box::SalsaBox::new(their_pubkey, my_secret);
    let nonce = crypto_box::generate_nonce(&mut OsRng);
    let ciphertext = salsa_box.encrypt(&nonce, body.as_bytes())?;
    Ok((ciphertext, nonce.into()))
}

/// Decrypt a DM
fn decrypt_dm(
    ciphertext: &[u8],
    nonce: &[u8; 24],
    my_secret: &StaticSecret,
    their_pubkey: &PublicKey,
) -> Result<String> {
    let salsa_box = crypto_box::SalsaBox::new(their_pubkey, my_secret);
    let plaintext = salsa_box.decrypt(nonce.into(), ciphertext)?;
    Ok(String::from_utf8(plaintext)?)
}
```

### Gossip Events

```rust
pub enum EventPayload {
    // ... existing variants ...
    
    /// Direct message between two peers
    DirectMessage {
        message_id: String,
        encrypted_body: Vec<u8>,
        nonce: [u8; 24],
        created_at: String,
    },
    
    /// Read receipt (optional)
    DmReadReceipt {
        message_ids: Vec<String>,
        read_at: String,
    },
}
```

### API Endpoints

```
GET  /conversations                    # List all conversations
GET  /conversations/:peer_id           # Get conversation with specific peer
GET  /conversations/:peer_id/messages  # Get messages in conversation
POST /conversations/:peer_id/messages  # Send a DM
POST /conversations/:peer_id/read      # Mark messages as read
```

### Implementation Tasks

1. [ ] Create `direct_messages` and `conversations` tables
2. [ ] Implement conversation ID derivation
3. [ ] Implement DM shared secret derivation
4. [ ] Implement DM topic derivation
5. [ ] Implement DM encryption/decryption
6. [ ] Add `DirectMessage` gossip event type
7. [ ] Create `DmService` with CRUD operations
8. [ ] Implement DM gossip publishing
9. [ ] Implement DM ingest (apply incoming DMs)
10. [ ] Subscribe to DM topics for all friends on startup
11. [ ] Add API endpoints for DM operations
12. [ ] Frontend: Conversations list view
13. [ ] Frontend: Conversation detail view
14. [ ] Frontend: Compose DM UI
15. [ ] Frontend: Unread indicators
16. [ ] Optional: Read receipts

---

## Part 4: Blocking & Blocklists

### Blocking Behavior

Blocking is binary — you either block someone or you don't. When you block a peer:

- Their posts are not displayed in your UI
- Their content is not stored locally (replaced with redacted placeholder)
- Their content is not relayed when you share threads
- DAG structure is preserved via placeholders

### Database Schema

```sql
-- Local blocks
CREATE TABLE blocked_peers (
    peer_id TEXT PRIMARY KEY,
    reason TEXT,                    -- Optional note
    blocked_at TEXT NOT NULL
);

-- Subscribed blocklists
CREATE TABLE blocklist_subscriptions (
    id TEXT PRIMARY KEY,            -- Blocklist ID (hash of maintainer + name)
    maintainer_peer_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    auto_apply BOOLEAN DEFAULT true,
    last_synced_at TEXT,
    FOREIGN KEY (maintainer_peer_id) REFERENCES peers(id)
);

-- Blocklist entries (cached from subscriptions)
CREATE TABLE blocklist_entries (
    blocklist_id TEXT NOT NULL,
    peer_id TEXT NOT NULL,
    reason TEXT,
    added_at TEXT NOT NULL,
    PRIMARY KEY (blocklist_id, peer_id),
    FOREIGN KEY (blocklist_id) REFERENCES blocklist_subscriptions(id)
);

-- Redacted post placeholders (for DAG preservation)
CREATE TABLE redacted_posts (
    id TEXT PRIMARY KEY,
    thread_id TEXT NOT NULL,
    author_peer_id TEXT NOT NULL,
    parent_post_ids TEXT NOT NULL,  -- JSON array
    known_child_ids TEXT,           -- JSON array, may be incomplete
    redaction_reason TEXT NOT NULL, -- 'blocked', 'not_included', 'deleted'
    discovered_at TEXT NOT NULL,
    FOREIGN KEY (thread_id) REFERENCES threads(id)
);
```

### Blocklist Data Structure

```rust
pub struct Blocklist {
    pub id: String,
    pub maintainer_peer_id: String,
    pub name: String,
    pub description: Option<String>,
    pub entries: Vec<BlocklistEntry>,
    pub updated_at: String,
    pub signature: String,  // Maintainer signs the list
}

pub struct BlocklistEntry {
    pub peer_id: String,
    pub reason: Option<String>,
    pub added_at: String,
}
```

### Redacted Post Model

```rust
pub enum PostContent {
    /// Full post content
    Full(PostRecord),
    
    /// Placeholder for blocked/missing content
    Redacted {
        id: String,
        thread_id: String,
        author_peer_id: String,
        parent_post_ids: Vec<String>,
        known_child_ids: Vec<String>,
        reason: RedactionReason,
    },
}

pub enum RedactionReason {
    /// Sender blocked this author
    BlockedBySender,
    /// Content exists but wasn't included (bandwidth, partial sync)
    NotIncluded,
    /// Author deleted the post
    Deleted,
    /// Local user has blocked this author
    BlockedLocally,
}
```

### Block Checking

```rust
pub struct BlockChecker {
    db: Database,
}

impl BlockChecker {
    /// Check if a peer is blocked (local blocks + subscribed lists)
    pub fn is_blocked(&self, peer_id: &str) -> Result<bool> {
        // Check local blocks first
        if self.get_local_block(peer_id)?.is_some() {
            return Ok(true);
        }
        
        // Check subscribed blocklists with auto_apply
        if self.is_on_subscribed_blocklist(peer_id)? {
            return Ok(true);
        }
        
        Ok(false)
    }
}
```

### Thread Blob with Redactions

When sharing a thread, replace blocked posts with redacted placeholders:

```rust
impl ThreadService {
    pub fn get_thread_for_sharing(
        &self,
        thread_id: &str,
        block_checker: &BlockChecker,
    ) -> Result<ThreadDetailsForSharing> {
        let thread = self.get_thread(thread_id)?;
        
        let posts: Vec<PostContent> = thread.posts
            .into_iter()
            .map(|post| {
                if !block_checker.is_blocked(&post.author_peer_id)? {
                    Ok(PostContent::Full(post))
                } else {
                    Ok(PostContent::Redacted {
                        id: post.id,
                        thread_id: post.thread_id,
                        author_peer_id: post.author_peer_id,
                        parent_post_ids: post.parent_post_ids,
                        known_child_ids: self.get_child_post_ids(&post.id)?,
                        reason: RedactionReason::BlockedBySender,
                    })
                }
            })
            .collect::<Result<Vec<_>>>()?;
        
        Ok(ThreadDetailsForSharing {
            thread: thread.thread,
            posts,
            peers: thread.peers,
            files: thread.files.into_iter()
                .filter(|f| !block_checker.is_blocked(&f.uploader_peer_id).unwrap_or(false))
                .collect(),
        })
    }
}
```

### Ingest with Block Checking

```rust
impl IngestWorker {
    async fn apply_post_update(&self, update: PostUpdate) -> Result<()> {
        let block_checker = BlockChecker::new(&self.db);
        
        // Check if author is blocked
        if block_checker.is_blocked(&update.post.author_peer_id)? {
            // Store as redacted placeholder to preserve DAG
            self.store_redacted_post(RedactedPost {
                id: update.post.id,
                thread_id: update.post.thread_id,
                author_peer_id: update.post.author_peer_id,
                parent_post_ids: update.post.parent_post_ids,
                known_child_ids: vec![],
                reason: RedactionReason::BlockedLocally,
            })?;
            return Ok(());
        }
        
        // Normal processing...
        self.post_repo.upsert(&update.post)?;
    }
}
```

### Blocklist Gossip

```rust
pub enum EventPayload {
    // ... existing variants ...
    
    /// Blocklist update from maintainer
    BlocklistUpdate {
        blocklist: Blocklist,
    },
}

// Blocklists are announced on the maintainer's peer topic
// Subscribers receive updates automatically
```

### API Endpoints

```
# Local blocking
GET    /blocks                      # List local blocks
POST   /blocks                      # Block a peer
DELETE /blocks/:peer_id             # Unblock a peer

# Blocklist subscriptions
GET    /blocklists                  # List subscribed blocklists
POST   /blocklists/subscribe        # Subscribe to a blocklist
DELETE /blocklists/:id              # Unsubscribe
GET    /blocklists/:id/entries      # View blocklist entries

# Publishing your own blocklist
POST   /blocklists/publish          # Create/update your blocklist
GET    /blocklists/mine             # Get your published blocklist
```

### Implementation Tasks

1. [ ] Create `blocked_peers` table
2. [ ] Create `blocklist_subscriptions` table
3. [ ] Create `blocklist_entries` table
4. [ ] Create `redacted_posts` table
5. [ ] Implement `BlockChecker` service
6. [ ] Update ingest to check blocks before storing
7. [ ] Implement redacted post placeholder storage
8. [ ] Update `get_thread` to return `PostContent` enum
9. [ ] Update thread blob serialization to include redacted posts
10. [ ] Implement blocklist data structures
11. [ ] Implement blocklist signing/verification
12. [ ] Add `BlocklistUpdate` gossip event
13. [ ] Implement blocklist subscription sync
14. [ ] Add blocking API endpoints
15. [ ] Add blocklist API endpoints
16. [ ] Frontend: Block user action (from post, profile)
17. [ ] Frontend: Blocked users management UI
18. [ ] Frontend: Blocklist subscription UI
19. [ ] Frontend: Redacted post placeholder display
20. [ ] Frontend: "Load blocked content" option for redacted posts

---

## Part 5: Migration Strategy

### Phase 1: Foundation (No Breaking Changes)

1. Add X25519 keypair generation (new nodes get it, existing nodes get it on next start)
2. Add new database columns with defaults
3. Friendcode v2 with backward compatibility (parse v1 or v2)

### Phase 2: Thread Visibility

1. Default new threads to `Social` visibility
2. Existing threads migrated to `Social` with generated topic_secret
3. Add visibility UI to frontend

### Phase 3: Private Threads & Encryption

1. Implement private thread creation
2. Implement encryption for private threads only
3. Add member management UI

### Phase 4: DMs

1. Implement DM backend
2. Add DM UI to frontend
3. DMs only work between peers with X25519 keys (friends added after update)

### Phase 5: Blocking

1. Implement local blocking
2. Implement redacted post handling
3. Add blocking UI
4. Implement blocklist subscriptions

---

## Security Considerations

### Key Storage

- X25519 secret key stored in `keys/encryption.key`
- File permissions should be 0600 (owner read/write only)
- Consider memory protection (mlock, zeroize on drop)

### Nonce Reuse

- Never reuse nonces with the same key
- Use random nonces (24 bytes for XSalsa20, 12 bytes for ChaCha20)
- For deterministic contexts, use HKDF to derive unique nonces

### Forward Secrecy

- This design does NOT provide forward secrecy
- If thread key is compromised, all messages are readable
- For truly sensitive use cases, recommend ephemeral threads
- Future enhancement: Consider ratcheting for DMs

### Metadata Leakage

- Topic subscriptions reveal interest in threads/conversations
- Consider Tor/I2P integration for network-level privacy
- Iroh relay usage may leak IP addresses

---

## Testing Strategy

### Unit Tests

- Key generation and serialization
- Encryption/decryption round-trips
- Topic derivation determinism
- Block checking logic
- Redacted post DAG preservation

### Integration Tests

- Two-node thread sharing with visibility levels
- DM exchange between two nodes
- Blocking prevents content relay
- Blocklist subscription and sync

### Manual Testing Scenarios

1. Create Social thread, share with friend, friend shares with their friend
2. Create Private thread, add members, verify non-members can't decrypt
3. Block a user, verify their posts become redacted
4. Subscribe to blocklist, verify auto-blocking works
5. Receive thread with redacted posts, fetch missing posts from another peer

---

## Open Questions

1. **Blocklist governance**: Should blocklists be signed by multiple maintainers?
2. **Key rotation**: How to handle X25519 key compromise/rotation?
3. **DM history**: Should DM history be synced across devices?
4. **Invite tokens**: Should Social threads use explicit invite tokens instead of just topic_secret?
5. **Spam prevention**: Rate limiting on thread creation? Proof of work?

---

## Appendix: Crate Versions

```toml
# Recommended versions (as of Dec 2024)
x25519-dalek = "2.0"
crypto_box = "0.9"
chacha20poly1305 = "0.10"
hkdf = "0.12"
sha2 = "0.10"
blake3 = "1.5"  # Already in use
rand = "0.8"    # Already in use
```

All crates are from the RustCrypto project and are well-maintained with security audits.
