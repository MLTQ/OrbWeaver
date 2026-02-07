# identity.rs

## Purpose
Local identity management including GPG keypair generation, Iroh secret key handling, and friend code encoding/decoding. Establishes the cryptographic identity used for authentication and encryption.

## Components

### `IdentitySummary`
- **Does**: Bundles all identity components
- **Fields**: gpg_fingerprint, iroh_peer_id, x25519_pubkey, friendcode, short_friendcode
- **Flags**: gpg_created, iroh_key_created, x25519_created (for first-run detection)

### `ensure_local_identity`
- **Does**: Ensures all identity components exist, creating if needed
- **Flow**: GPG → Iroh → X25519 → Generate friend codes
- **Returns**: Complete `IdentitySummary`

### `FriendCodePayload`
- **Does**: Structured data encoded in friend codes
- **Fields**: version, peer_id, gpg_fingerprint, x25519_pubkey, addresses

## GPG Identity

### `ensure_gpg_identity`
- **Does**: Loads existing or generates new GPG keypair
- **Storage**: `keys/gpg/fingerprint.txt`, `private.asc`, `public.asc`
- **Algorithm**: Ed25519 (signing) + Cv25519 (encryption) via sequoia-openpgp

### `generate_gpg_identity`
- **Does**: Creates new GPG certificate with unique user ID
- **User ID**: "Graphchan Node {uuid} <node-{uuid}@graphchan.local>"

## Iroh Identity

### `ensure_iroh_identity`
- **Does**: Loads existing or generates new Iroh secret key
- **Storage**: `keys/iroh.key` (JSON with base64 encoded secret)
- **Key type**: Iroh SecretKey for P2P networking

### `load_iroh_secret`
- **Does**: Reads Iroh secret key from disk
- **Returns**: `iroh_base::SecretKey`

## Friend Codes

### `encode_friendcode`
- **Does**: Creates shareable friend code with network addresses
- **Format**: Base64(zstd(JSON(FriendCodePayload)))
- **Includes**: peer_id, gpg_fingerprint, x25519_pubkey, multiaddrs

### `encode_short_friendcode`
- **Does**: Creates compact friend code without addresses
- **Format**: Base58(peer_id + fingerprint)
- **Use case**: Display-friendly, addresses added at share time

### `decode_friendcode_auto`
- **Does**: Decodes both full and short friend code formats
- **Returns**: `FriendCodePayload`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `bootstrap.rs` | `ensure_local_identity(paths)` | Signature change |
| `peers.rs` | `decode_friendcode_auto` | Format changes |
| `dms.rs` | X25519 key available | Key format changes |

## Storage Layout

```
keys/
├── gpg/
│   ├── fingerprint.txt   # GPG fingerprint (hex)
│   ├── private.asc       # Armored private key
│   └── public.asc        # Armored public key
├── iroh.key              # JSON {version, peer_id, secret_key_b64}
└── encryption.key        # X25519 keypair (from crypto/keys.rs)
```

## Notes
- Permissions tightened to 0o600 on Unix for private keys
- GPG fingerprint serves as global peer identifier
- Friend codes enable peer discovery without central server
- X25519 used for DM encryption (separate from GPG)
