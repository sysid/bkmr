use std::env;
use anyhow::{anyhow, Result, Context as _};
use serde_derive::{Deserialize, Serialize};
use tracing::{debug, instrument};
use super::Embedding;

#[derive(Debug, Clone, Default)]
pub struct DummyEmbedding;

impl Embedding for DummyEmbedding {
    #[instrument]
    fn embed(&self, text: &str) -> Result<Option<Vec<f32>>> {
        debug!("DummyEmbedding::embed({})", text);
        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiEmbedding {
    url: String,
}

impl Default for OpenAiEmbedding {
    fn default() -> Self {
        Self {
            url: "https://api.openai.com".to_string(),
        }
    }
}

#[derive(Serialize)]
struct EmbeddingRequest {
    input: String,
    model: String,
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
}

impl Embedding for OpenAiEmbedding {
    #[instrument]
    fn embed(&self, text: &str) -> Result<Option<Vec<f32>>> {
        debug!("OpenAI embedding request for: {}", text);
        let client = reqwest::blocking::Client::new();
        let api_key = env::var("OPENAI_API_KEY").context("OPENAI_API_KEY not set")?;

        let request = EmbeddingRequest {
            input: text.to_string(),
            model: "text-embedding-ada-002".to_string(),
        };

        let response = client
            .post(format!("{}/v1/embeddings", self.url))
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&request)
            .send()?
            .json::<EmbeddingResponse>()
            .context("Failed to parse OpenAI response")?;

        response.data.first()
            .map(|data| data.embedding.clone())
            .ok_or_else(|| anyhow!("No embeddings in response"))
            .map(Some)
    }
}

impl OpenAiEmbedding {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}