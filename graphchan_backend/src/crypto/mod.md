# mod.rs (crypto module)

## Purpose
Cryptographic primitives for encryption, key derivation, and secure messaging. Provides X25519 key management, DM encryption, and thread encryption for private content.

## Submodules

- **keys** - X25519 keypair management
- **utils** - Key derivation and nonce generation
- **dm_crypto** - Direct message encryption/decryption
- **thread_crypto** - Private thread encryption

## Re-exports

### From keys
- `X25519Identity` - Secret + public key pair
- `WrappedKey` - Encrypted key with nonce
- `ensure_x25519_identity` - Create/load keypair
- `load_x25519_secret` - Load secret key

### From utils
- `derive_key` - HKDF key derivation
- `generate_nonce_12` - 12-byte nonces (ChaCha20)
- `generate_nonce_24` - 24-byte nonces (XSalsa20)

### From dm_crypto
- `encrypt_dm` - Encrypt direct message
- `decrypt_dm` - Decrypt direct message
- `derive_dm_shared_secret` - X25519 DH shared secret

### From thread_crypto
- `encrypt_thread_blob` - Encrypt thread content
- `decrypt_thread_blob` - Decrypt thread content
- `derive_file_key` - Per-file key derivation
- `wrap_thread_key` / `unwrap_thread_key` - Key encryption for members

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `identity.rs` | `ensure_x25519_identity` | Function changes |
| `dms.rs` | DM encrypt/decrypt functions | API changes |
| `network/ingest.rs` | Thread decryption | Format changes |

## Cryptographic Algorithms

| Purpose | Algorithm | Library |
|---------|-----------|---------|
| DH Key Exchange | X25519 | x25519-dalek |
| DM Encryption | XSalsa20Poly1305 | crypto_box |
| Thread Encryption | ChaCha20Poly1305 | chacha20poly1305 |
| Key Derivation | HKDF-SHA256 | hkdf |
| Hashing | Blake3 | blake3 |

## Notes
- All functions use constant-time operations where applicable
- Nonces are randomly generated for each encryption
- Key wrapping enables secure key distribution to group members
- X25519 keys separate from GPG keys (different use cases)
