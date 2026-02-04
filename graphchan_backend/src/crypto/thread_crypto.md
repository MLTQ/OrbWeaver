# thread_crypto.rs

## Purpose
Encryption functions for private thread content using ChaCha20Poly1305, and key wrapping for distributing thread keys to authorized members.

## Components

### `encrypt_thread_blob`
- **Does**: Encrypts thread JSON blob with thread key
- **Inputs**: plaintext bytes, 32-byte thread_key
- **Returns**: nonce (12 bytes) + ciphertext
- **Algorithm**: ChaCha20Poly1305

### `decrypt_thread_blob`
- **Does**: Decrypts thread blob
- **Inputs**: encrypted (nonce + ciphertext), thread_key
- **Returns**: Plaintext bytes
- **Validates**: Minimum length for nonce

### `derive_file_key`
- **Does**: Derives per-file encryption key from thread key
- **Inputs**: thread_key, file_id
- **Algorithm**: HKDF with context "orbweaver-file-v1:{file_id}"
- **Returns**: 32-byte file key

### `wrap_thread_key`
- **Does**: Encrypts thread key for a specific member
- **Inputs**: thread_key, recipient_pubkey, sender_secret
- **Returns**: `WrappedKey` (ciphertext + nonce)
- **Algorithm**: crypto_box (XSalsa20Poly1305)

### `unwrap_thread_key`
- **Does**: Decrypts a wrapped thread key
- **Inputs**: wrapped key, sender_pubkey, recipient_secret
- **Returns**: 32-byte thread key

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `network/ingest.rs` | `decrypt_thread_blob` | Format changes |
| `files.rs` | `derive_file_key` for file encryption | Algorithm changes |

## Key Hierarchy

```
Thread Key (32 bytes, random)
    │
    ├── encrypt_thread_blob()  → Thread content
    │
    ├── derive_file_key(thread_key, file_id)
    │       │
    │       └── encrypt_file()  → File content
    │
    └── wrap_thread_key(thread_key, member_pubkey)
            │
            └── WrappedKey  → Stored per-member
```

## Wire Format

### Encrypted Thread Blob
```
[12-byte nonce][ciphertext][16-byte Poly1305 tag]
```

### Wrapped Key
```json
{
  "ciphertext": "base64...",
  "nonce": [24 bytes]
}
```

## Notes
- Thread key generated once per thread, shared with members
- File keys derived deterministically (same key for re-encryption)
- Key wrapping enables adding new members without re-encrypting content
- ChaCha20 chosen for software performance (no AES-NI required)
