pub mod dedupe;
pub mod structural;
pub mod tokenizer;

pub use dedupe::{compute_chunk_hash, SimHash};
pub use structural::{ChunkingConfig, StructuralChunker};
pub use tokenizer::{ApproximateTokenizer, TokenCounter, WordTokenizer};
