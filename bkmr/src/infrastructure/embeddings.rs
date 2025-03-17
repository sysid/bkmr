// src/infrastructure/embeddings.rs

use std::fs::File;
use std::io::Read;
use ndarray::Array1;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::debug;

use crate::domain::error::{DomainError, DomainResult};

pub trait Embedding: Send + Sync {
    fn embed(&self, text: &str) -> DomainResult<Option<Vec<f32>>>;
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmbeddingWrapper {
    pub values: Vec<f32>,
}

/// Dummy embedding implementation that always returns None
pub struct DummyEmbedding;

impl Embedding for DummyEmbedding {
    fn embed(&self, _text: &str) -> DomainResult<Option<Vec<f32>>> {
        Ok(None)
    }
}

pub struct OpenAiEmbedding {
    api_key: String,
    model: String,
}

impl Default for OpenAiEmbedding {
    fn default() -> Self {
        let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_default();
        Self {
            api_key,
            model: "text-embedding-ada-002".to_string(),
        }
    }
}

#[derive(Error, Debug)]
pub enum EmbeddingError {
    #[error("Failed to serialize embedding: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("OpenAI API error: {0}")]
    ApiError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    HttpError(#[from] reqwest::Error),
}

impl From<EmbeddingError> for DomainError {
    fn from(err: EmbeddingError) -> Self {
        DomainError::CannotFetchMetadata(err.to_string())
    }
}

#[derive(Deserialize)]
struct OpenAiEmbeddingResponse {
    data: Vec<OpenAiEmbeddingData>,
}

#[derive(Deserialize)]
struct OpenAiEmbeddingData {
    embedding: Vec<f32>,
}

impl Embedding for OpenAiEmbedding {
    fn embed(&self, text: &str) -> DomainResult<Option<Vec<f32>>> {
        if self.api_key.is_empty() {
            debug!("OpenAI API key not set, skipping embedding");
            return Ok(None);
        }

        let client = reqwest::blocking::Client::new();

        let payload = serde_json::json!({
            "input": text,
            "model": self.model,
        });

        let response = client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .map_err(|e| DomainError::CannotFetchMetadata(e.to_string()))?; // Fixed conversion

        if !response.status().is_success() {
            let error_text = response.text()
                .map_err(|e| DomainError::CannotFetchMetadata(e.to_string()))?;
            return Err(DomainError::CannotFetchMetadata(error_text));
        }

        let response_data: OpenAiEmbeddingResponse = response.json()
            .map_err(|e| DomainError::CannotFetchMetadata(e.to_string()))?;

        if response_data.data.is_empty() {
            debug!("Empty response from OpenAI API");
            return Ok(None);
        }

        Ok(Some(response_data.data[0].embedding.clone()))
    }
}

pub fn serialize_embedding(embedding: Vec<f32>) -> Result<Vec<u8>, EmbeddingError> {
    let wrapper = EmbeddingWrapper { values: embedding };
    let serialized = serde_json::to_vec(&wrapper)?;
    Ok(serialized)
}

pub fn deserialize_embedding(bytes: Vec<u8>) -> Result<Vec<f32>, EmbeddingError> {
    let wrapper: EmbeddingWrapper = serde_json::from_slice(&bytes)?;
    Ok(wrapper.values)
}

pub fn cosine_similarity(a: &Array1<f32>, b: &Array1<f32>) -> f32 {
    let dot_product = a.dot(b);
    let norm_a = a.dot(a).sqrt();
    let norm_b = b.dot(b).sqrt();

    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }

    dot_product / (norm_a * norm_b)
}

pub fn load_json_embedding_from_file(path: &str) -> Result<Vec<f32>, EmbeddingError> {
    let mut file = File::open(path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    let embedding: Vec<f32> = serde_json::from_str(&contents)?;
    Ok(embedding)
}
