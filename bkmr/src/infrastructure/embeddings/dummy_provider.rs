use crate::domain::embedding::Embedder;
use crate::domain::error::DomainResult;
use tracing::{debug, instrument};

/// Dummy implementation that always returns None.
/// Used in tests and for non-semantic code paths where embedding
/// initialization is unnecessary.
#[derive(Debug, Clone, Default)]
pub struct DummyEmbedding;

impl Embedder for DummyEmbedding {
    #[instrument]
    fn embed_document(&self, _text: &str) -> DomainResult<Option<Vec<f32>>> {
        debug!("DummyEmbedding::embed_document() called - returns None");
        Ok(None)
    }

    #[instrument]
    fn embed_query(&self, _text: &str) -> DomainResult<Option<Vec<f32>>> {
        debug!("DummyEmbedding::embed_query() called - returns None");
        Ok(None)
    }

    fn dimensions(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn given_text_input_when_embed_document_then_returns_none() {
        let dummy = DummyEmbedding;
        let result = dummy.embed_document("test text").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn given_text_input_when_embed_query_then_returns_none() {
        let dummy = DummyEmbedding;
        let result = dummy.embed_query("test query").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn given_dummy_when_dimensions_then_returns_zero() {
        let dummy = DummyEmbedding;
        assert_eq!(dummy.dimensions(), 0);
    }
}
