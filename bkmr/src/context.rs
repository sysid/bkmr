// bkmr/src/context.rs
use crate::adapter::embeddings::{serialize_embedding, DummyEmbedding, Embedding};
use anyhow::Result;
use once_cell::sync::OnceCell;
use std::fmt;
use std::sync::RwLock;

pub static CTX: OnceCell<RwLock<Context>> = OnceCell::new();

pub struct Context {
    embedder: Box<dyn Embedding>,
}

impl fmt::Debug for Context {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Context")
            .field("embedder", &"Box<dyn Embedding>")
            .finish()
    }
}

impl Context {
    pub fn new(embedder: Box<dyn Embedding>) -> Self {
        Self { embedder }
    }

    pub fn execute(&self, text: &str) -> Result<Option<Vec<f32>>> {
        self.embedder.embed(text)
    }

    /// Gets embedding for text and serializes it to bytes
    pub fn get_embedding(&self, content: &str) -> Option<Vec<u8>> {
        match self.execute(content) {
            Ok(Some(embedding)) => match serialize_embedding(embedding) {
                Ok(bytes) => Some(bytes),
                Err(e) => {
                    eprintln!("Error serializing embedding: {}", e);
                    None
                }
            },
            Ok(None) => None,
            Err(e) => {
                eprintln!("Error generating embedding: {}", e);
                None
            }
        }
    }

    pub fn global() -> &'static RwLock<Context> {
        CTX.get_or_init(|| RwLock::new(Context::new(Box::new(DummyEmbedding))))
    }

    pub fn read_global() -> std::sync::RwLockReadGuard<'static, Context> {
        Self::global()
            .read()
            .expect("Failed to acquire context read lock")
    }

    pub fn update_global(new_context: Context) -> Result<()> {
        let mut context = Self::global()
            .write()
            .map_err(|e| anyhow::anyhow!("Failed to acquire context write lock: {}", e))?;
        *context = new_context;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use rstest::*;
    use serial_test::serial;
    

    // Mock embedder that always succeeds
    struct SuccessEmbedding;
    impl Embedding for SuccessEmbedding {
        fn embed(&self, _text: &str) -> Result<Option<Vec<f32>>> {
            Ok(Some(vec![0.1, 0.2, 0.3]))
        }
    }

    // Mock embedder that always returns None
    struct NoneEmbedding;
    impl Embedding for NoneEmbedding {
        fn embed(&self, _text: &str) -> Result<Option<Vec<f32>>> {
            Ok(None)
        }
    }

    // Mock embedder that always fails
    struct FailingEmbedding;
    impl Embedding for FailingEmbedding {
        fn embed(&self, _text: &str) -> Result<Option<Vec<f32>>> {
            Err(anyhow::anyhow!("Embedding failed"))
        }
    }

    #[fixture]
    fn success_context() -> Context {
        Context::new(Box::new(SuccessEmbedding))
    }

    #[fixture]
    fn none_context() -> Context {
        Context::new(Box::new(NoneEmbedding))
    }

    #[fixture]
    fn failing_context() -> Context {
        Context::new(Box::new(FailingEmbedding))
    }

    #[rstest]
    fn test_execute_success(success_context: Context) {
        let result = success_context.execute("test text");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(vec![0.1, 0.2, 0.3]));
    }

    #[rstest]
    fn test_execute_none(none_context: Context) {
        let result = none_context.execute("test text");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), None);
    }

    #[rstest]
    fn test_execute_failure(failing_context: Context) {
        let result = failing_context.execute("test text");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Embedding failed");
    }

    #[rstest]
    fn test_get_embedding_success(success_context: Context) {
        let result = success_context.get_embedding("test text");
        assert!(result.is_some());
        // Verify the serialized bytes match expected format
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
    }

    #[rstest]
    fn test_get_embedding_none(none_context: Context) {
        let result = none_context.get_embedding("test text");
        assert!(result.is_none());
    }

    #[rstest]
    fn test_get_embedding_failure(failing_context: Context) {
        let result = failing_context.get_embedding("test text");
        assert!(result.is_none());
    }

    #[test]
    fn test_global_initialization() {
        let ctx = Context::global();
        assert!(ctx.read().is_ok());
    }

    #[test]
    #[serial]
    fn test_read_global() -> Result<()> {
        Context::update_global(Context::new(Box::new(DummyEmbedding)))?;
        let ctx = Context::read_global();
        // Verify we can access the default DummyEmbedding context
        let result = ctx.execute("test text");
        assert!(result.is_ok());
        assert_eq!(result?, None); // DummyEmbedding returns None
        Ok(())
    }

    #[test]
    #[serial]
    fn test_update_global() {
        // Reset context at start to ensure clean state
        {
            let dummy_ctx = Context::new(Box::new(DummyEmbedding));
            Context::update_global(dummy_ctx).unwrap();
        }

        // Run test
        let success_ctx = Context::new(Box::new(SuccessEmbedding));
        assert!(Context::update_global(success_ctx).is_ok());

        let ctx = Context::read_global();
        let result = ctx.execute("test text");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Some(vec![0.1, 0.2, 0.3]));

        // Reset context back to default state after test
        drop(ctx); // Explicitly drop read lock before updating
        let dummy_ctx = Context::new(Box::new(DummyEmbedding));
        assert!(Context::update_global(dummy_ctx).is_ok());
    }

    #[test]
    fn test_debug_implementation() {
        let ctx = Context::new(Box::new(DummyEmbedding));
        let debug_output = format!("{:?}", ctx);
        assert!(debug_output.contains("Context"));
        assert!(debug_output.contains("embedder"));
    }

}
