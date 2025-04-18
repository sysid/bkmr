use crate::domain::embedding::Embedder;
use crate::domain::error::DomainResult;
use std::any::Any;
use tracing::{debug, instrument};

/// Dummy implementation that always returns None
#[derive(Debug, Clone, Default)]
pub struct DummyEmbedding;

impl Embedder for DummyEmbedding {
    #[instrument]
    fn embed(&self, _text: &str) -> DomainResult<Option<Vec<f32>>> {
        debug!("DummyEmbedding::embed() called - returns None");
        Ok(None)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_dummy_embedding_returns_none() {
        let dummy = DummyEmbedding;
        let result = dummy.embed("test text").unwrap();
        assert!(result.is_none());
    }
}
