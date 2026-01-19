use blake3;

/// The global discovery topic that all nodes subscribe to by default.
/// Global threads are broadcast on this topic for public discovery.
/// DEPRECATED: Use user-defined topics instead.
pub const GLOBAL_TOPIC_NAME: &str = "graphchan-global-v1";

/// Derives the topic ID for global threads.
/// Global threads use a well-known topic that all nodes subscribe to.
/// DEPRECATED: Use derive_topic_id() with custom topic names instead.
pub fn derive_global_topic() -> [u8; 32] {
    *blake3::hash(GLOBAL_TOPIC_NAME.as_bytes()).as_bytes()
}

/// Derives a topic ID from a topic name.
/// Topic IDs are deterministic blake3 hashes of "topic:{name}".
/// Anyone who knows the topic name can subscribe to it.
pub fn derive_topic_id(topic_name: &str) -> [u8; 32] {
    let input = format!("topic:{}", topic_name);
    *blake3::hash(input.as_bytes()).as_bytes()
}

/// Derives the gossip topic for a social thread.
/// Social threads use a secret topic derived from thread_id + topic_secret.
pub fn derive_social_thread_topic(thread_id: &str, topic_secret: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"orbweaver-social-v1:");
    hasher.update(thread_id.as_bytes());
    hasher.update(b":");
    hasher.update(topic_secret);
    *hasher.finalize().as_bytes()
}

/// Derives the gossip topic for a private thread.
/// Private threads also use secret topics (like social), but additionally
/// encrypt the thread content.
pub fn derive_private_thread_topic(thread_id: &str, topic_secret: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"orbweaver-private-v1:");
    hasher.update(thread_id.as_bytes());
    hasher.update(b":");
    hasher.update(topic_secret);
    *hasher.finalize().as_bytes()
}

/// Convenience function to derive a thread topic based on visibility.
pub fn derive_thread_topic(thread_id: &str, visibility: &str, topic_secret: &[u8; 32]) -> [u8; 32] {
    match visibility {
        "global" => derive_global_topic(),
        "private" => derive_private_thread_topic(thread_id, topic_secret),
        "social" => derive_social_thread_topic(thread_id, topic_secret),
        _ => derive_social_thread_topic(thread_id, topic_secret), // default to social
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_social_topic_deterministic() {
        let thread_id = "thread-123";
        let secret = [42u8; 32];

        let topic1 = derive_social_thread_topic(thread_id, &secret);
        let topic2 = derive_social_thread_topic(thread_id, &secret);

        assert_eq!(topic1, topic2);
    }

    #[test]
    fn test_private_topic_deterministic() {
        let thread_id = "thread-456";
        let secret = [99u8; 32];

        let topic1 = derive_private_thread_topic(thread_id, &secret);
        let topic2 = derive_private_thread_topic(thread_id, &secret);

        assert_eq!(topic1, topic2);
    }

    #[test]
    fn test_different_secrets_different_topics() {
        let thread_id = "thread-789";
        let secret1 = [1u8; 32];
        let secret2 = [2u8; 32];

        let topic1 = derive_social_thread_topic(thread_id, &secret1);
        let topic2 = derive_social_thread_topic(thread_id, &secret2);

        assert_ne!(topic1, topic2);
    }

    #[test]
    fn test_social_vs_private_different_topics() {
        let thread_id = "thread-abc";
        let secret = [77u8; 32];

        let social_topic = derive_social_thread_topic(thread_id, &secret);
        let private_topic = derive_private_thread_topic(thread_id, &secret);

        assert_ne!(social_topic, private_topic);
    }

    #[test]
    fn test_derive_thread_topic_social() {
        let thread_id = "thread-def";
        let secret = [33u8; 32];

        let social1 = derive_social_thread_topic(thread_id, &secret);
        let social2 = derive_thread_topic(thread_id, "social", &secret);

        assert_eq!(social1, social2);
    }

    #[test]
    fn test_derive_thread_topic_private() {
        let thread_id = "thread-ghi";
        let secret = [55u8; 32];

        let private1 = derive_private_thread_topic(thread_id, &secret);
        let private2 = derive_thread_topic(thread_id, "private", &secret);

        assert_eq!(private1, private2);
    }

    #[test]
    fn test_derive_thread_topic_defaults_to_social() {
        let thread_id = "thread-jkl";
        let secret = [88u8; 32];

        let social = derive_social_thread_topic(thread_id, &secret);
        let unknown = derive_thread_topic(thread_id, "unknown", &secret);

        assert_eq!(social, unknown);
    }
}
