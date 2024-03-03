use std::io::Cursor;
use std::{env, fmt};

use anyhow::anyhow;
use anyhow::Context as anyhowContext;
pub use anyhow::Result;
use bincode::{deserialize, serialize};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use log::debug;
use serde_derive::{Deserialize, Serialize};

use crate::dlog2;

#[derive(Serialize)]
struct EmbeddingPayload {
    input: String,
    model: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
    // Include other fields from the response if necessary
    // model: String,
    // object: String,
    // usage: ...
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    // index: usize,
    // object: String,
    // Add any other fields here if needed
}

// #[derive(Clone, PartialEq, Eq)]
pub struct Context {
    strategy: Box<dyn Embedding>,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("strategy", &"Box<dyn Embedding>")
            .finish()
    }
}

impl Context {
    pub fn new(strategy: Box<dyn Embedding>) -> Self {
        Context { strategy }
    }

    pub fn execute(&self, text: &str) -> Result<Option<Vec<f32>>> {
        self.strategy.get_openai_embedding(text)
    }

    /// Get the embedding for a text using the correct strategy.
    /// Internal error handling in order to allow for a graceful fallback
    pub fn get_embedding(&self, content: &str) -> Option<Vec<u8>> {
        
        match self.execute(content) {
            Ok(maybe_embd) => maybe_embd.and_then(|embd| {
                match serialize_embedding(embd) {
                    Ok(serialized) => Some(serialized),
                    Err(e) => {
                        eprintln!("Error during serialization: {}", e);
                        None // Choose to continue with None in case of an error
                    }
                }
            }),
            Err(e) => {
                eprintln!("Error fetching embeddings: {}", e);
                None // Choose to continue with None in case of an error
            }
        }
    }
}

pub trait Embedding: Sync + Send {
    fn get_openai_embedding(&self, text: &str) -> Result<Option<Vec<f32>>>;
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DummyAi;

impl DummyAi {
    pub fn new() -> Self {
        DummyAi {}
    }
}

impl Embedding for DummyAi {
    fn get_openai_embedding(&self, text: &str) -> Result<Option<Vec<f32>>> {
        debug!("DummyAi::get_openai_embedding({})", text);
        Ok(None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenAi {
    url: String,
}

impl Default for OpenAi {
    fn default() -> Self {
        OpenAi {
            url: "https://api.openai.com".to_string(),
        }
    }
}

impl Embedding for OpenAi {
    /// Get the embedding for a text using the OpenAI API.
    /// max tokens: 8191, dim: 1536
    fn get_openai_embedding(&self, text: &str) -> Result<Option<Vec<f32>>> {
        dlog2!("OpenAi get embedding for: {}", text);
        let client = reqwest::blocking::Client::new();
        let api_key = env::var("OPENAI_API_KEY").expect("OPENAI_API_KEY not set");

        let payload = EmbeddingPayload {
            input: text.to_string(),
            model: "text-embedding-ada-002".to_string(),
        };

        let response = client
            .post(format!("{}/v1/embeddings", self.url))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&payload)
            .send()?
            .json::<EmbeddingResponse>()
            .with_context(|| format!("OpenAPI request failed: {}", api_key))?;

        // Ensure we have data and it contains at least one embedding
        let embedding_data = response
            .data
            .get(0)
            .ok_or_else(|| anyhow!("No embeddings found"))?;
        Ok(Some(embedding_data.embedding.clone()))
    }
}

impl OpenAi {
    // Constructs a new instance of `OpenAi` with a specific URL
    pub fn new(url: String) -> Self {
        OpenAi { url }
    }
}

pub fn cosine_similarity(vec1: &ndarray::Array1<f32>, vec2: &ndarray::Array1<f32>) -> f32 {
    let dot_product = vec1.dot(vec2);
    let magnitude_vec1 = vec1.dot(vec1).sqrt();
    let magnitude_vec2 = vec2.dot(vec2).sqrt();

    // To prevent division by zero
    if magnitude_vec1 == 0.0 || magnitude_vec2 == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_vec1 * magnitude_vec2)
}

/// Deserialize a byte array into a vector of floats
pub fn deserialize_embedding(serialized: Vec<u8>) -> Result<Vec<f32>> {
    deserialize(&serialized).map_err(|e| anyhow!("Failed to deserialize embedding: {}", e))
}

/// Serialize a vector of floats into a byte array
pub fn serialize_embedding(embedding: Vec<f32>) -> Result<Vec<u8>> {
    serialize(&embedding).map_err(|e| anyhow!("Failed to serialize embedding: {}", e))
}

pub fn embedding2array(blob: &[u8]) -> ndarray::Array1<f32> {
    let mut cursor = Cursor::new(blob);
    let num_floats = blob.len() / 4; // Since each f32 is 4 bytes
    let mut array = Vec::with_capacity(num_floats);

    for _ in 0..num_floats {
        let num = cursor.read_f32::<LittleEndian>().unwrap();
        array.push(num);
    }

    ndarray::Array1::from(array)
}

#[allow(dead_code)]
fn array2embedding(embedding: &ndarray::Array1<f32>) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(embedding.len() * 4);
    for &value in embedding {
        buffer.write_f32::<LittleEndian>(value).unwrap();
    }
    buffer
}

#[cfg(test)]
mod tests {
    use bincode::deserialize;
    use ndarray::{array, Array1};
    use rstest::*;

    use super::*;

    #[ctor::ctor]
    fn init() {
        let _ = env_logger::builder()
            // Include all events in tests
            .filter_level(log::LevelFilter::max())
            // Ensure events are captured by `cargo test`
            .is_test(true)
            // Ignore errors initializing the logger if tests race to configure it
            .try_init();
    }

    #[rstest]
    fn test_cosine_similarity() {
        let vec1 = array![1.0, 0.0];
        let vec2 = array![0.0, 1.0];
        let similarity = cosine_similarity(&vec1, &vec2);

        // Cosine similarity between orthogonal vectors should be 0.
        assert_eq!(similarity, 0.0);
    }

    #[test]
    fn test_serialize_embedding_successful() {
        let embedding: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
        let serialized = serialize_embedding(embedding.clone()).unwrap();
        let deserialized: Vec<f32> = deserialize(&serialized).unwrap();

        assert_eq!(embedding, deserialized);
    }

    #[test]
    fn test_serialize_deserialize_round_trip() {
        let original_embedding = vec![1.0, 2.0, 3.0];
        let bytes = serialize_embedding(original_embedding.clone()).unwrap();
        let deserialized_embedding = deserialize_embedding(bytes);

        assert_eq!(deserialized_embedding.unwrap(), original_embedding);
    }

    #[test]
    fn test_embedding_round_trip() {
        let original_embedding = Array1::from(vec![1.0f32, 2.0, 3.0, 4.0]);
        let blob = array2embedding(&original_embedding);
        let reconstructed_embedding = embedding2array(&blob);

        assert_eq!(original_embedding, reconstructed_embedding);
    }

    #[test]
    fn test_embedding2array() {
        let blob: Vec<u8> = vec![
            0, 0, 128, 63, // 1.0f32 in little-endian
            0, 0, 0, 64, // 2.0f32 in little-endian
            0, 0, 64, 64, // 3.0f32 in little-endian
            0, 0, 128, 64,
        ]; // 4.0f32 in little-endian
        let expected_array = Array1::from(vec![1.0f32, 2.0, 3.0, 4.0]);
        let result_array = embedding2array(&blob);

        assert_eq!(expected_array, result_array);
    }

    #[test]
    fn test_array2embedding() {
        let array = Array1::from(vec![1.0f32, 2.0, 3.0, 4.0]);
        let expected_blob: Vec<u8> = vec![
            0, 0, 128, 63, // 1.0f32 in little-endian
            0, 0, 0, 64, // 2.0f32 in little-endian
            0, 0, 64, 64, // 3.0f32 in little-endian
            0, 0, 128, 64,
        ]; // 4.0f32 in little-endian
        let result_blob = array2embedding(&array);

        assert_eq!(expected_blob, result_blob);
    }
}
