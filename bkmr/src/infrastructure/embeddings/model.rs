use serde_derive::{Deserialize, Serialize};

#[derive(Serialize)]
pub struct EmbeddingRequest {
    pub(crate) input: String,
    pub(crate) model: String,
}

#[derive(Deserialize)]
pub struct EmbeddingResponse {
    pub(crate) data: Vec<EmbeddingData>,
}

#[derive(Deserialize)]
pub struct EmbeddingData {
    pub(crate) embedding: Vec<f32>,
}
