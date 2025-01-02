mod providers;
mod utils;

pub use providers::{DummyEmbedding, OpenAiEmbedding};
pub use utils::{cosine_similarity, deserialize_embedding, serialize_embedding};

use anyhow::Result;

/// Core trait for text embedding functionality
pub trait Embedding: Send + Sync {
    /// Embeds text into a vector of floats
    fn embed(&self, text: &str) -> Result<Option<Vec<f32>>>;
}