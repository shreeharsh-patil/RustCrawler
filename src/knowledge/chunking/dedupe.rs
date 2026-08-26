use blake3::Hasher;

/// Computes a deterministic BLAKE3 hash over normalized chunk text
pub fn compute_chunk_hash(text: &str) -> String {
    let normalized = normalize_text_for_hashing(text);
    let mut hasher = Hasher::new();
    hasher.update(normalized.as_bytes());
    hasher.finalize().to_hex().to_string()
}

/// Normalizes text by collapsing whitespace and trimming
fn normalize_text_for_hashing(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut in_whitespace = false;
    for c in text.trim().chars() {
        if c.is_whitespace() {
            if !in_whitespace {
                result.push(' ');
                in_whitespace = true;
            }
        } else {
            result.push(c);
            in_whitespace = false;
        }
    }
    result
}

/// 64-bit SimHash for near-duplicate text detection without requiring neural embeddings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SimHash(pub u64);

impl SimHash {
    /// Computes 64-bit SimHash over word shingles of the text
    pub fn compute(text: &str) -> Self {
        let words: Vec<&str> = text.split_whitespace().collect();
        if words.is_empty() {
            return SimHash(0);
        }

        let mut v = [0i32; 64];

        // 1. Unigrams with higher weight
        for word in &words {
            let h = hash_token(word);
            update_vector(&mut v, h, 2);
        }

        // 2. Bigram shingles for phrase context
        if words.len() >= 2 {
            for window in words.windows(2) {
                let shingle = format!("{} {}", window[0], window[1]);
                let h = hash_token(&shingle);
                update_vector(&mut v, h, 1);
            }
        }

        // 2. Form 64-bit fingerprint: bit i is 1 if v[i] > 0, else 0
        let mut fingerprint: u64 = 0;
        for (i, val) in v.iter().enumerate() {
            if *val > 0 {
                fingerprint |= 1u64 << i;
            }
        }

        SimHash(fingerprint)
    }

    /// Computes Hamming distance (number of differing bits) between two SimHashes
    pub fn hamming_distance(&self, other: &SimHash) -> u32 {
        (self.0 ^ other.0).count_ones()
    }

    /// Checks if two texts are near-duplicates using a conservative Hamming distance threshold (default <= 3)
    pub fn is_near_duplicate(&self, other: &SimHash, max_distance: u32) -> bool {
        self.hamming_distance(other) <= max_distance
    }
}

fn hash_token(token: &str) -> u64 {
    let mut hasher = Hasher::new();
    hasher.update(token.to_lowercase().as_bytes());
    let hash_bytes = hasher.finalize();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&hash_bytes.as_bytes()[0..8]);
    u64::from_le_bytes(bytes)
}

fn update_vector(v: &mut [i32; 64], hash: u64, weight: i32) {
    for (i, item) in v.iter_mut().enumerate() {
        let bit = (hash >> i) & 1;
        if bit == 1 {
            *item += weight;
        } else {
            *item -= weight;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_hash_deterministic_and_whitespace_invariant() {
        let text1 = "  Hello   world!\nThis is a test.  ";
        let text2 = "Hello world! This is a test.";
        let hash1 = compute_chunk_hash(text1);
        let hash2 = compute_chunk_hash(text2);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_simhash_near_duplicates() {
        let original = "Rust is a multi-paradigm, general-purpose programming language that emphasizes performance, type safety, and concurrency.";
        let slightly_modified = "Rust is a multi-paradigm, general-purpose language that emphasizes high performance, type safety, and concurrency.";
        let completely_different = "Chocolate cake recipes require flour, sugar, cocoa powder, baking soda, eggs, and milk.";

        let sim1 = SimHash::compute(original);
        let sim2 = SimHash::compute(slightly_modified);
        let sim3 = SimHash::compute(completely_different);

        let dist_near = sim1.hamming_distance(&sim2);
        let dist_diff = sim1.hamming_distance(&sim3);

        assert!(
            dist_near < dist_diff,
            "Near duplicate distance ({dist_near}) must be smaller than different text distance ({dist_diff})"
        );
        assert!(
            dist_near <= 12,
            "Slightly modified text should have low Hamming distance ({dist_near})"
        );
        assert!(
            dist_diff >= 18,
            "Different text should have high Hamming distance ({dist_diff})"
        );
    }
}
