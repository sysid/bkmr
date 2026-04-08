use crate::domain::embedding::Embedder;
use crate::domain::error::{DomainError, DomainResult};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use std::sync::Mutex;
use tracing::{debug, instrument};

/// Local embedding provider using fastembed with ONNX Runtime.
/// Default model: NomicEmbedTextV15 (768 dimensions).
///
/// The ONNX model is loaded lazily on first embed call to avoid blocking
/// startup for commands that don't need embeddings.
///
/// Uses `search_document:` and `search_query:` prefixes per the Nomic
/// model protocol.
pub struct FastEmbedEmbedding {
    /// The model is initialized lazily on first embed call.
    model: Mutex<Option<TextEmbedding>>,
    embedding_model: EmbeddingModel,
    dims: usize,
}

impl std::fmt::Debug for FastEmbedEmbedding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FastEmbedEmbedding")
            .field("dims", &self.dims)
            .field("initialized", &self.model.lock().map(|g| g.is_some()).unwrap_or(false))
            .finish()
    }
}

impl FastEmbedEmbedding {
    /// Create a new FastEmbedEmbedding with the given model.
    /// The ONNX model is NOT loaded here — it is loaded lazily on first
    /// embed call. This keeps ServiceContainer creation fast.
    pub fn new(embedding_model: EmbeddingModel) -> Self {
        let dims = Self::model_dimensions(&embedding_model);
        debug!(
            "Created FastEmbedEmbedding config for {:?}, dims={}",
            embedding_model, dims
        );
        Self {
            model: Mutex::new(None),
            embedding_model,
            dims,
        }
    }

    /// Create with the default model (NomicEmbedTextV15).
    pub fn default_model() -> Self {
        Self::new(EmbeddingModel::NomicEmbedTextV15)
    }

    /// Resolve the model enum variant from a config string.
    pub fn model_from_name(name: &str) -> DomainResult<EmbeddingModel> {
        match name {
            "NomicEmbedTextV15" => Ok(EmbeddingModel::NomicEmbedTextV15),
            "NomicEmbedTextV15Q" => Ok(EmbeddingModel::NomicEmbedTextV15Q),
            "AllMiniLML6V2" => Ok(EmbeddingModel::AllMiniLML6V2),
            "AllMiniLML6V2Q" => Ok(EmbeddingModel::AllMiniLML6V2Q),
            "BGESmallENV15" => Ok(EmbeddingModel::BGESmallENV15),
            "BGESmallENV15Q" => Ok(EmbeddingModel::BGESmallENV15Q),
            "BGEM3" => Ok(EmbeddingModel::BGEM3),
            _ => Err(DomainError::ConfigurationError(format!(
                "Unknown embedding model: '{}'. Supported: NomicEmbedTextV15, NomicEmbedTextV15Q, \
                 AllMiniLML6V2, AllMiniLML6V2Q, BGESmallENV15, BGESmallENV15Q, BGEM3",
                name
            ))),
        }
    }

    fn model_dimensions(model: &EmbeddingModel) -> usize {
        match model {
            EmbeddingModel::NomicEmbedTextV15 | EmbeddingModel::NomicEmbedTextV15Q => 768,
            EmbeddingModel::AllMiniLML6V2 | EmbeddingModel::AllMiniLML6V2Q => 384,
            EmbeddingModel::BGESmallENV15 | EmbeddingModel::BGESmallENV15Q => 384,
            EmbeddingModel::BGEM3 => 1024,
            _ => 768, // conservative default
        }
    }

    /// Returns the model cache directory path.
    pub fn cache_dir() -> String {
        std::env::var("FASTEMBED_CACHE_DIR").unwrap_or_else(|_| {
            dirs::cache_dir()
                .map(|d| d.join("bkmr").join("models").to_string_lossy().to_string())
                .unwrap_or_else(|| ".fastembed_cache".to_string())
        })
    }

    fn embed_with_prefix(&self, text: &str, prefix: &str) -> DomainResult<Option<Vec<f32>>> {
        let prefixed = format!("{}{}", prefix, text);
        let mut guard = self.model.lock().map_err(|e| {
            DomainError::BookmarkOperationFailed(format!("Embedding model lock poisoned: {}", e))
        })?;

        // Lazy init: load the ONNX model on first use
        if guard.is_none() {
            let cache_dir = Self::cache_dir();
            let cache_path = std::path::Path::new(&cache_dir);
            let needs_download = !cache_path.exists() || cache_path.read_dir().map_or(true, |mut d| d.next().is_none());

            if needs_download {
                eprintln!(
                    "Downloading embedding model {:?} (one-time)...",
                    self.embedding_model
                );
                eprintln!("Cache location: {}", cache_dir);
            } else {
                eprintln!("Loading embedding model {:?}...", self.embedding_model);
            }

            debug!(
                "Lazy-loading ONNX model {:?}, cache={}",
                self.embedding_model, cache_dir
            );
            let options = TextInitOptions::new(self.embedding_model.clone())
                .with_cache_dir(cache_dir.into())
                .with_show_download_progress(true);
            let model = TextEmbedding::try_new(options).map_err(|e| {
                DomainError::ConfigurationError(format!(
                    "Failed to initialize embedding model: {}",
                    e
                ))
            })?;
            *guard = Some(model);
        }

        let model = guard.as_mut().expect("model just initialized");
        let results = model.embed(vec![&prefixed], None).map_err(|e| {
            DomainError::BookmarkOperationFailed(format!("Embedding generation failed: {}", e))
        })?;
        Ok(results.into_iter().next())
    }
}

impl Embedder for FastEmbedEmbedding {
    #[instrument(skip(self))]
    fn embed_document(&self, text: &str) -> DomainResult<Option<Vec<f32>>> {
        self.embed_with_prefix(text, "search_document: ")
    }

    #[instrument(skip(self))]
    fn embed_query(&self, text: &str) -> DomainResult<Option<Vec<f32>>> {
        self.embed_with_prefix(text, "search_query: ")
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}
