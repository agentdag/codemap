//! Content hashing for incremental change detection.

/// blake3 content hash of a source string, hex-encoded. Used for incremental
/// change detection (a node's hash covers its source slice; a `File` node's
/// hash covers the whole file contents).
pub fn content_hash(source: &str) -> String {
    blake3::hash(source.as_bytes()).to_hex().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_blake3_hex_and_stable() {
        let h = content_hash("hello");
        // blake3 hex is 64 chars.
        assert_eq!(h.len(), 64);
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(h, content_hash("hello"));
        assert_ne!(h, content_hash("world"));
        // Known blake3 vector for "hello".
        assert_eq!(
            h,
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f"
        );
    }
}
