pub(crate) mod dummy_provider;
mod model;
pub mod openai_provider;

pub use dummy_provider::DummyEmbedding;
pub use openai_provider::OpenAiEmbedding;
