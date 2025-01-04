use camino::Utf8PathBuf;
use rstest::*;
use std::env;

use bkmr::adapter::dal::migration;
use bkmr::adapter::dal::Dal;
use bkmr::adapter::embeddings::{Embedding, OpenAiEmbedding};
use bkmr::context::CTX;
use bkmr::service::embeddings::create_embeddings_for_non_bookmarks;
use bkmr::util::testing::test_dal;
use tracing::debug;

#[fixture]
fn test_data_path() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/resources/data.ndjson")
}

#[rstest]
fn given_ndjson_file_when_creating_embeddings_for_new_bookmarks_then_succeeds(
    _test_dal: Dal,
    test_data_path: Utf8PathBuf,
) {
    // Arrange

    // Act
    let result = create_embeddings_for_non_bookmarks(&test_data_path);

    // Assert
    assert!(result.is_ok(), "Failed to create embeddings: {:?}", result);
}

#[rstest]
fn given_existing_embeddings_when_creating_again_then_succeeds(
    _test_dal: Dal,
    test_data_path: Utf8PathBuf,
) {
    // Arrange
    let first_run = create_embeddings_for_non_bookmarks(&test_data_path);
    assert!(first_run.is_ok(), "First run failed: {:?}", first_run);

    // Act
    let result = create_embeddings_for_non_bookmarks(&test_data_path);

    // Assert
    assert!(result.is_ok(), "Second run failed: {:?}", result);
}

// Add helper test to verify context is properly set
#[test]
fn given_application_when_initializing_then_context_exists() {
    assert!(CTX.get().is_some(), "Global context should be initialized");
}

#[rstest]
fn given_mock_openai_api_when_requesting_embedding_then_returns_vector() {
    // Given: Request a new server from the pool
    let mut server = mockito::Server::new();
    // Use one of these addresses to configure your client
    let url = server.url();
    // Set the environment variable to use the mock server URL
    env::set_var("OPENAI_API_KEY", "test_key");
    // Create a mock
    let _m = server
        .mock("POST", "/v1/embeddings")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [{"embedding": [0.1, 0.2, 0.3]}]}"#)
        .create();
    debug!("{:?}", url);

    let open_ai = OpenAiEmbedding::new(url);
    let input_text = "example text";

    // When: Get the embedding
    let embedding = open_ai.embed(input_text).unwrap().unwrap();
    // Then: Ensure the embedding is correct
    assert_eq!(embedding, vec![0.1, 0.2, 0.3]);

    // Cleanup
    env::remove_var("OPENAI_API_KEY")
}
