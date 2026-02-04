# keys.rs

## Purpose
X25519 keypair management for encryption operations. Handles key generation, storage, and loading with proper file permissions.

## Components

### `X25519Identity`
- **Does**: Holds X25519 secret and public key pair
- **Fields**: `secret: StaticSecret`, `public: PublicKey`
- **Debug**: Public key shown, secret redacted

### `WrappedKey`
- **Does**: Encrypted symmetric key with nonce
- **Fields**: `ciphertext: Vec<u8>`, `nonce: [u8; 24]`
- **Use case**: Distributing thread keys to members

### `ensure_x25519_identity`
- **Does**: Creates or loads X25519 keypair
- **Storage**: `keys/encryption.key` (JSON)
- **Returns**: `(public_key_base64, was_created)`

### `load_x25519_secret`
- **Does**: Loads full identity from disk
- **Returns**: `X25519Identity` with both keys
- **Errors**: If file missing or malformed

## Storage Format

```json
{
  "version": 1,
  "public_key_b64": "base64...",
  "secret_key_b64": "base64..."
}
```

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `identity.rs` | `ensure_x25519_identity(paths)` | Signature change |
| `dms.rs` | `load_x25519_secret(paths)` | Return type change |
| `thread_crypto.rs` | `WrappedKey` struct | Field changes |

## Security

- File permissions set to 0o600 on Unix (owner read/write only)
- Secret key never logged (Debug impl redacts)
- Random key generation via `rand::rng().fill_bytes()`

## Notes
- X25519 chosen for modern ECC with good performance
- Separate from GPG keys to use different crypto libraries
- StaticSecret/PublicKey from x25519-dalek
- Base64 encoding for JSON storage compatibility
