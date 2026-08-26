/// Trait for counting tokens in text strings
pub trait TokenCounter: Send + Sync {
    fn count(&self, text: &str) -> usize;
}

/// Fast approximate tokenizer using ~4 characters per token heuristic with word-boundary awareness
#[derive(Debug, Clone, Default)]
pub struct ApproximateTokenizer;

impl TokenCounter for ApproximateTokenizer {
    fn count(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        // Words + punctuation estimate: typically 1 token = 0.75 words, or ~4 chars
        let chars = text.chars().count();
        let words = text.split_whitespace().count();
        if words == 0 {
            return 1;
        }
        // Weighted blend of character length and word count for good approximation
        let from_chars = chars.div_ceil(4);
        let from_words = (words * 4).div_ceil(3);
        (from_chars + from_words) / 2
    }
}

/// Word-based tokenizer counting whitespace-separated words plus punctuation tokens
#[derive(Debug, Clone, Default)]
pub struct WordTokenizer;

impl TokenCounter for WordTokenizer {
    fn count(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        let words = text.split_whitespace().count();
        // Add estimate for special symbols, code tokens, and punctuation
        let punctuation_count = text
            .chars()
            .filter(|c| c.is_ascii_punctuation() && *c != ' ')
            .count();
        words + (punctuation_count / 3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_approximate_tokenizer() {
        let tokenizer = ApproximateTokenizer;
        assert_eq!(tokenizer.count(""), 0);
        let sample = "Hello world! This is a test of the approximate token counter in Rust.";
        let count = tokenizer.count(sample);
        assert!(
            (10..=25).contains(&count),
            "Token count {} should be reasonable",
            count
        );
    }

    #[test]
    fn test_word_tokenizer() {
        let tokenizer = WordTokenizer;
        assert_eq!(tokenizer.count(""), 0);
        let sample = "fn main() { println!(\"Hello, world!\"); }";
        let count = tokenizer.count(sample);
        assert!(count >= 5, "Token count should capture code tokens");
    }
}
