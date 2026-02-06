# network/schelling.rs

## Purpose
Schelling point discovery for topic-based peer finding via BEP44 mutable DHT records. Solves the problem where two strangers on the same topic can't discover each other because DTT records only contain `node_id` (no relay URL or direct addresses).

## How It Works

### Key Insight
All peers who know a topic name can derive the same BEP44 signing key for a given minute window. This creates a "Schelling point" — a coordination point that requires no prior agreement beyond knowing the topic name.

### Key Derivation
1. `topic_hash = SHA512("topic:" + name)` → 64 bytes
2. `signing_seed = HKDF-SHA256(topic_hash[..32], info=minute_le_bytes)` → 32 bytes → ed25519 SigningKey
3. `salt = blake3(topic_name)` → BEP44 salt parameter
4. `encryption_key = HKDF-SHA256(topic_hash[32..64], info="schelling-encrypt")` → 32 bytes

### Record Format
`SchellingRecord` serialized as JSON, then encrypted with ChaCha20Poly1305:
- `node_id`: iroh PublicKey (hex string)
- `relay_url`: Optional relay URL from `endpoint.addr()`
- `direct_addrs`: Direct socket addresses from `endpoint.addr()`

### Discovery Loop (`run_schelling_loop`)
Every 30 seconds:
1. Query BEP44 for current minute and previous minute slots
2. Decrypt records → extract SchellingRecord
3. Skip own node_id
4. For each new peer: inject EndpointAddr into StaticProvider + add to gossip topic
5. Publish own record to current minute slot

## Components

### `SchellingRecord`
- **Does**: Holds endpoint addressing info for DHT publication
- **Serialized as**: JSON → encrypted → BEP44 value (must be < 1000 bytes)

### `derive_signing_key(topic, minute) → SigningKey`
- **Does**: Deterministic ed25519 key from topic + time window
- **Property**: Same inputs → same key on all peers

### `derive_encryption_key(topic) → [u8; 32]`
- **Does**: Topic-specific ChaCha20Poly1305 key
- **Property**: Independent from signing key (different halves of SHA-512)

### `encrypt_record / decrypt_record`
- **Does**: ChaCha20Poly1305 with 12-byte random nonce prepended
- **Pattern**: Same as `encrypt_thread_blob` in crypto module

### `publish_record / query_records`
- **Does**: BEP44 put/get via `mainline::Dht` (blocking, run via `block_in_place`)
- **Note**: Last-writer-wins on same (key, salt) — acceptable with 2-slot querying

## Design Decisions
- **Per-minute rotation**: Limits DHT pollution; stale records expire naturally
- **Encryption**: Prevents passive DHT observers from reading peer addresses
- **StaticProvider injection**: Correct iroh 0.95 API for out-of-band address injection
- **Relay-independent**: Records contain whatever the endpoint has (relay, direct, or both)
- **No new dependencies**: Uses mainline, ed25519-dalek, chacha20poly1305, hkdf, sha2, blake3 (all already in Cargo.toml)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `network.rs` | `run_schelling_loop()` signature | Function params |
| `network.rs` | Uses `topics::derive_topic_id` for gossip topic ID | Topic derivation changes |

## Notes
- DHT operations are blocking (`mainline::Dht`), wrapped with `tokio::task::block_in_place`
- BEP44 value limit is 1000 bytes; typical encrypted record is ~200-300 bytes
- Runs alongside DTT discovery (both active simultaneously)
- `known_peers` HashSet prevents redundant StaticProvider injections for already-seen peers
- Addresses are refreshed each cycle even for known peers (relay/IP may change)
