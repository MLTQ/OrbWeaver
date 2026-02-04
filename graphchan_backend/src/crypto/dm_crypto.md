# dm_crypto.rs

## Purpose
Cryptographic functions for direct message encryption using crypto_box (XSalsa20Poly1305 with X25519 key exchange).

## Components

### `encrypt_dm`
- **Does**: Encrypts a DM body for a recipient
- **Inputs**: body (plaintext), sender_secret, recipient_pubkey
- **Returns**: `(ciphertext, nonce)`
- **Algorithm**: XSalsa20Poly1305 via crypto_box::SalsaBox

### `decrypt_dm`
- **Does**: Decrypts a received DM
- **Inputs**: ciphertext, nonce, recipient_secret, sender_pubkey
- **Returns**: Plaintext string
- **Validates**: UTF-8 validity of decrypted content

### `derive_dm_shared_secret`
- **Does**: Derives deterministic shared secret for topic derivation
- **Inputs**: my_secret, their_pubkey
- **Algorithm**: X25519 DH → HKDF with context "orbweaver-dm-secret-v1"
- **Returns**: 32-byte secret

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `dms.rs` | `encrypt_dm`, `decrypt_dm` signatures | Parameter changes |
| `network/topics.rs` | `derive_dm_shared_secret` output | Algorithm changes |

## Encryption Flow

```
Sender:
1. X25519 DH: shared = sender_secret * recipient_pubkey
2. SalsaBox derives symmetric key from shared secret
3. Generate random 24-byte nonce
4. Encrypt: ciphertext = XSalsa20Poly1305(key, nonce, plaintext)
5. Send (ciphertext, nonce)

Recipient:
1. X25519 DH: shared = recipient_secret * sender_pubkey
2. SalsaBox derives same symmetric key
3. Decrypt: plaintext = XSalsa20Poly1305(key, nonce, ciphertext)
```

## Notes
- crypto_box handles ECDH + symmetric encryption in one API
- Nonce must be unique per message (random 24 bytes)
- Same shared secret for both directions (symmetric DH)
- Poly1305 tag provides authentication
