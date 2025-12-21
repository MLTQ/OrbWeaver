use super::utils::{derive_key, generate_nonce_12, generate_nonce_24};
use super::keys::WrappedKey;
use anyhow::Result;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use x25519_dalek::{PublicKey, StaticSecret};

/// Encrypts a thread blob (JSON bytes) for private threads using ChaCha20Poly1305.
pub fn encrypt_thread_blob(plaintext: &[u8], thread_key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(thread_key.into());
    let nonce_bytes = generate_nonce_12();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("encryption failed: {}", e))?;

    // Prepend nonce to ciphertext
    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);

    Ok(result)
}

/// Decrypts a thread blob using ChaCha20Poly1305.
pub fn decrypt_thread_blob(encrypted: &[u8], thread_key: &[u8; 32]) -> Result<Vec<u8>> {
    if encrypted.len() < 12 {
        anyhow::bail!("encrypted data too short");
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = ChaCha20Poly1305::new(thread_key.into());

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("decryption failed: {}", e))
}

/// Derives a file encryption key from the thread key and file ID.
pub fn derive_file_key(thread_key: &[u8; 32], file_id: &str) -> [u8; 32] {
    let info = format!("orbweaver-file-v1:{}", file_id);
    let derived = derive_key(thread_key, info.as_bytes(), 32);

    let mut key = [0u8; 32];
    key.copy_from_slice(&derived);
    key
}

/// Wraps (encrypts) a thread key for a specific member using crypto_box.
pub fn wrap_thread_key(
    thread_key: &[u8; 32],
    recipient_pubkey: &PublicKey,
    sender_secret: &StaticSecret,
) -> Result<WrappedKey> {
    // Convert x25519-dalek types to crypto_box types
    let cb_secret = crypto_box::SecretKey::from(sender_secret.to_bytes());
    let cb_public = crypto_box::PublicKey::from(*recipient_pubkey.as_bytes());

    let salsa_box = crypto_box::SalsaBox::new(&cb_public, &cb_secret);
    let nonce_bytes = generate_nonce_24();
    let nonce = crypto_box::Nonce::from(nonce_bytes);

    let ciphertext = salsa_box
        .encrypt(&nonce, thread_key.as_ref())
        .map_err(|e| anyhow::anyhow!("key wrapping failed: {}", e))?;

    Ok(WrappedKey {
        ciphertext,
        nonce: nonce_bytes,
    })
}

/// Unwraps (decrypts) a thread key using crypto_box.
pub fn unwrap_thread_key(
    wrapped: &WrappedKey,
    sender_pubkey: &PublicKey,
    recipient_secret: &StaticSecret,
) -> Result<[u8; 32]> {
    // Convert x25519-dalek types to crypto_box types
    let cb_secret = crypto_box::SecretKey::from(recipient_secret.to_bytes());
    let cb_public = crypto_box::PublicKey::from(*sender_pubkey.as_bytes());

    let salsa_box = crypto_box::SalsaBox::new(&cb_public, &cb_secret);
    let nonce = crypto_box::Nonce::from(wrapped.nonce);

    let plaintext = salsa_box
        .decrypt(&nonce, wrapped.ciphertext.as_ref())
        .map_err(|e| anyhow::anyhow!("key unwrapping failed: {}", e))?;

    if plaintext.len() != 32 {
        anyhow::bail!("unwrapped key has invalid length: {}", plaintext.len());
    }

    let mut key = [0u8; 32];
    key.copy_from_slice(&plaintext);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let plaintext = b"Hello, private thread!";

        let encrypted = encrypt_thread_blob(plaintext, &key).unwrap();
        let decrypted = decrypt_thread_blob(&encrypted, &key).unwrap();

        assert_eq!(plaintext.as_ref(), decrypted);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let plaintext = b"secret data";

        let encrypted = encrypt_thread_blob(plaintext, &key1).unwrap();
        let result = decrypt_thread_blob(&encrypted, &key2);

        assert!(result.is_err());
    }

    #[test]
    fn test_derive_file_key_deterministic() {
        let thread_key = [99u8; 32];
        let file_id = "file123";

        let key1 = derive_file_key(&thread_key, file_id);
        let key2 = derive_file_key(&thread_key, file_id);

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_derive_file_key_different_ids() {
        let thread_key = [99u8; 32];

        let key1 = derive_file_key(&thread_key, "file1");
        let key2 = derive_file_key(&thread_key, "file2");

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_wrap_unwrap_thread_key() {
        let thread_key = [77u8; 32];

        let mut sender_bytes = [0u8; 32];
        let mut recipient_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut sender_bytes);
        rand::rng().fill_bytes(&mut recipient_bytes);
        let sender_secret = StaticSecret::from(sender_bytes);
        let recipient_secret = StaticSecret::from(recipient_bytes);

        let sender_public = PublicKey::from(&sender_secret);
        let recipient_public = PublicKey::from(&recipient_secret);

        // Sender wraps the key for recipient
        let wrapped = wrap_thread_key(&thread_key, &recipient_public, &sender_secret).unwrap();

        // Recipient unwraps the key using sender's public key
        let unwrapped = unwrap_thread_key(&wrapped, &sender_public, &recipient_secret).unwrap();

        assert_eq!(thread_key, unwrapped);
    }

    #[test]
    fn test_unwrap_with_wrong_recipient_fails() {
        let thread_key = [77u8; 32];

        let mut sender_bytes = [0u8; 32];
        let mut recipient1_bytes = [0u8; 32];
        let mut recipient2_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut sender_bytes);
        rand::rng().fill_bytes(&mut recipient1_bytes);
        rand::rng().fill_bytes(&mut recipient2_bytes);
        let sender_secret = StaticSecret::from(sender_bytes);
        let recipient1_secret = StaticSecret::from(recipient1_bytes);
        let recipient2_secret = StaticSecret::from(recipient2_bytes);

        let sender_public = PublicKey::from(&sender_secret);
        let recipient1_public = PublicKey::from(&recipient1_secret);

        let wrapped = wrap_thread_key(&thread_key, &recipient1_public, &sender_secret).unwrap();

        // Different recipient tries to unwrap
        let result = unwrap_thread_key(&wrapped, &sender_public, &recipient2_secret);
        assert!(result.is_err());
    }
}
