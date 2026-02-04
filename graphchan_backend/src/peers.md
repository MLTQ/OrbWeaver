# peers.rs

## Purpose
Peer management service handling friend code registration, profile updates, and peer discovery. Bridges between friend codes and database peer records.

## Components

### `PeerService`
- **Does**: CRUD operations for peer records
- **Interacts with**: `Database`, `PeerRepository`, `PeerIpRepository`

### `list_peers`
- **Does**: Lists all known peers
- **Returns**: `Vec<PeerView>` converted from database records

### `get_local_peer`
- **Does**: Fetches or creates local identity peer record
- **Interacts with**: `database.get_identity()`, creates stub if missing
- **Returns**: `PeerView` for local user

### `register_friendcode`
- **Does**: Decodes friend code and creates/updates peer record
- **Interacts with**: `decode_friendcode_auto`, `PeerRepository.upsert`
- **Also**: Extracts and stores IP addresses from multiaddrs

### `update_profile`
- **Does**: Updates peer's avatar, username, bio, or agent list
- **Interacts with**: `PeerRepository.upsert`
- **Used by**: Profile updates from gossip, local edits

### `unfollow_peer`
- **Does**: Sets peer trust_state to "unfollowed"
- **Note**: Doesn't delete - preserves for post attribution

### `lookup_peer_by_ip`
- **Does**: Finds peer by known IP address
- **Interacts with**: `PeerIpRepository`
- **Use case**: Identifying connecting peers

## Data Types

### `PeerView`
- **Fields**: id, username, bio, friendcode, short_friendcode, gpg_fingerprint, trust_state, avatar_url, agents
- **Conversion**: `from_record(PeerRecord)`

### `FriendCodePayload`
- **Fields**: iroh_peer_id, gpg_fingerprint, x25519_pubkey, addresses (multiaddrs)
- **Decoded from**: Friend code string

## Helper Functions

### `payload_to_peer_record`
- **Does**: Converts decoded friend code to database record
- **Sets**: All fields from payload, trust_state = "trusted"

### `extract_ips_from_multiaddrs`
- **Does**: Parses IP addresses from multiaddr strings
- **Returns**: `Vec<IpAddr>` for IP blocking/lookup

### `encode_short_friendcode`
- **Does**: Generates compact friend code (without addresses)
- **Use case**: Display-friendly codes, addresses added at sharing time

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api.rs` | All service methods | Method signature changes |
| `network/events.rs` | `update_profile` for gossip updates | Parameter changes |
| `blocking.rs` | `lookup_peer_by_ip` for IP blocks | Return type changes |

## Notes
- Peer ID is GPG fingerprint (globally unique)
- Friend codes encode: iroh peer ID, GPG fingerprint, X25519 key, network addresses
- IP addresses stored separately for reverse lookup
- Trust states: "trusted" (followed), "unknown" (discovered), "unfollowed"
