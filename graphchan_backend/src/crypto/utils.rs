use hkdf::Hkdf;
use sha2::Sha256;

/// Derives a key using HKDF-SHA256.
pub fn derive_key(input_key: &[u8], info: &[u8], output_len: usize) -> Vec<u8> {
    let hk = Hkdf::<Sha256>::new(None, input_key);
    let mut output = vec![0u8; output_len];
    hk.expand(info, &mut output).expect("invalid HKDF length");
    output
}

/// Generates a random 12-byte nonce for ChaCha20Poly1305.
pub fn generate_nonce_12() -> [u8; 12] {
    use rand::RngCore;
    let mut nonce = [0u8; 12];
    rand::rng().fill_bytes(&mut nonce);
    nonce
}

/// Generates a random 24-byte nonce for crypto_box (XSalsa20Poly1305).
pub fn generate_nonce_24() -> [u8; 24] {
    use rand::RngCore;
    let mut nonce = [0u8; 24];
    rand::rng().fill_bytes(&mut nonce);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_key_deterministic() {
        let input = b"test input key material";
        let info = b"test context";

        let key1 = derive_key(input, info, 32);
        let key2 = derive_key(input, info, 32);

        assert_eq!(key1, key2);
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn test_derive_key_different_info() {
        let input = b"test input key material";

        let key1 = derive_key(input, b"context1", 32);
        let key2 = derive_key(input, b"context2", 32);

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_nonce_generation() {
        let nonce1 = generate_nonce_12();
        let nonce2 = generate_nonce_12();

        // Should be different (statistically)
        assert_ne!(nonce1, nonce2);
        assert_eq!(nonce1.len(), 12);

        let nonce3 = generate_nonce_24();
        let nonce4 = generate_nonce_24();

        assert_ne!(nonce3, nonce4);
        assert_eq!(nonce3.len(), 24);
    }
}
