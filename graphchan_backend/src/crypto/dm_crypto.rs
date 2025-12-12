use super::utils::{derive_key, generate_nonce_24};
use anyhow::Result;
use crypto_box::aead::Aead;
use x25519_dalek::{PublicKey as X25519PublicKey, StaticSecret as X25519StaticSecret};

/// Encrypts a direct message using crypto_box (XSalsa20Poly1305).
pub fn encrypt_dm(
    body: &str,
    sender_secret: &X25519StaticSecret,
    recipient_pubkey: &X25519PublicKey,
) -> Result<(Vec<u8>, [u8; 24])> {
    // Convert x25519-dalek types to crypto_box types
    let cb_secret = crypto_box::SecretKey::from(sender_secret.to_bytes());
    let cb_public = crypto_box::PublicKey::from(*recipient_pubkey.as_bytes());

    let salsa_box = crypto_box::SalsaBox::new(&cb_public, &cb_secret);
    let nonce_bytes = generate_nonce_24();
    let nonce = crypto_box::Nonce::from(nonce_bytes);

    let ciphertext = salsa_box
        .encrypt(&nonce, body.as_bytes())
        .map_err(|e| anyhow::anyhow!("DM encryption failed: {}", e))?;

    Ok((ciphertext, nonce_bytes))
}

/// Decrypts a direct message using crypto_box.
pub fn decrypt_dm(
    ciphertext: &[u8],
    nonce: &[u8; 24],
    recipient_secret: &X25519StaticSecret,
    sender_pubkey: &X25519PublicKey,
) -> Result<String> {
    // Convert x25519-dalek types to crypto_box types
    let cb_secret = crypto_box::SecretKey::from(recipient_secret.to_bytes());
    let cb_public = crypto_box::PublicKey::from(*sender_pubkey.as_bytes());

    let salsa_box = crypto_box::SalsaBox::new(&cb_public, &cb_secret);
    let nonce = crypto_box::Nonce::from(*nonce);

    let plaintext = salsa_box
        .decrypt(&nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("DM decryption failed: {}", e))?;

    String::from_utf8(plaintext).map_err(|e| anyhow::anyhow!("invalid UTF-8 in decrypted DM: {}", e))
}

/// Derives a shared secret for DM topic derivation using X25519 Diffie-Hellman.
pub fn derive_dm_shared_secret(my_secret: &X25519StaticSecret, their_pubkey: &X25519PublicKey) -> [u8; 32] {
    let shared = my_secret.diffie_hellman(their_pubkey);
    let derived = derive_key(shared.as_bytes(), b"orbweaver-dm-secret-v1", 32);

    let mut secret = [0u8; 32];
    secret.copy_from_slice(&derived);
    secret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_dm() {
        let mut sender_bytes = [0u8; 32];
        let mut recipient_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut sender_bytes);
        rand::rng().fill_bytes(&mut recipient_bytes);
        let sender_secret = X25519StaticSecret::from(sender_bytes);
        let recipient_secret = X25519StaticSecret::from(recipient_bytes);

        let sender_public = X25519PublicKey::from(&sender_secret);
        let recipient_public = X25519PublicKey::from(&recipient_secret);

        let message = "Hello, this is a private message!";

        // Sender encrypts
        let (ciphertext, nonce) = encrypt_dm(message, &sender_secret, &recipient_public).unwrap();

        // Recipient decrypts
        let decrypted = decrypt_dm(&ciphertext, &nonce, &recipient_secret, &sender_public).unwrap();

        assert_eq!(message, decrypted);
    }

    #[test]
    fn test_decrypt_with_wrong_recipient_fails() {
        let mut sender_bytes = [0u8; 32];
        let mut recipient1_bytes = [0u8; 32];
        let mut recipient2_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut sender_bytes);
        rand::rng().fill_bytes(&mut recipient1_bytes);
        rand::rng().fill_bytes(&mut recipient2_bytes);
        let sender_secret = X25519StaticSecret::from(sender_bytes);
        let recipient1_secret = X25519StaticSecret::from(recipient1_bytes);
        let recipient2_secret = X25519StaticSecret::from(recipient2_bytes);

        let sender_public = X25519PublicKey::from(&sender_secret);
        let recipient1_public = X25519PublicKey::from(&recipient1_secret);

        let message = "Secret message";
        let (ciphertext, nonce) = encrypt_dm(message, &sender_secret, &recipient1_public).unwrap();

        // Wrong recipient tries to decrypt
        let result = decrypt_dm(&ciphertext, &nonce, &recipient2_secret, &sender_public);
        assert!(result.is_err());
    }

    #[test]
    fn test_derive_dm_shared_secret_symmetric() {
        let mut alice_bytes = [0u8; 32];
        let mut bob_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut alice_bytes);
        rand::rng().fill_bytes(&mut bob_bytes);
        let alice_secret = X25519StaticSecret::from(alice_bytes);
        let bob_secret = X25519StaticSecret::from(bob_bytes);

        let alice_public = X25519PublicKey::from(&alice_secret);
        let bob_public = X25519PublicKey::from(&bob_secret);

        // Both parties should derive the same shared secret
        let secret_from_alice = derive_dm_shared_secret(&alice_secret, &bob_public);
        let secret_from_bob = derive_dm_shared_secret(&bob_secret, &alice_public);

        assert_eq!(secret_from_alice, secret_from_bob);
    }

    #[test]
    fn test_dm_shared_secret_deterministic() {
        let mut alice_bytes = [0u8; 32];
        let mut bob_bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut alice_bytes);
        rand::rng().fill_bytes(&mut bob_bytes);
        let alice_secret = X25519StaticSecret::from(alice_bytes);
        let bob_public = X25519PublicKey::from(&X25519StaticSecret::from(bob_bytes));

        let secret1 = derive_dm_shared_secret(&alice_secret, &bob_public);
        let secret2 = derive_dm_shared_secret(&alice_secret, &bob_public);

        assert_eq!(secret1, secret2);
    }
}
