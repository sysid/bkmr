use bkmr::adapter::embeddings::{Embedding, OpenAi};
use log::{debug, info};
use rstest::rstest;
use std::env;
use stdext::function_name;

#[ctor::ctor]
fn init() {
    let _ = env_logger::builder()
        // Include all events in tests
        .filter_level(log::LevelFilter::max())
        .filter_module("mio", log::LevelFilter::Info)
        .filter_module("reqwest", log::LevelFilter::Info)
        // Ensure events are captured by `cargo test`
        .is_test(true)
        // Ignore errors initializing the logger if tests race to configure it
        .try_init();
}

#[rstest]
fn test_xxx() {
    info!("xxx");
    debug!("xxx");
    assert_eq!(1, 1);
}

#[rstest]
fn test_get_openai_embedding() {
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
    debug!("({}:{}) {:?}", function_name!(), line!(), url);

    let open_ai = OpenAi::new(url);
    let input_text = "example text";

    // When: Get the embedding
    let embedding = open_ai.get_openai_embedding(input_text).unwrap().unwrap();
    // Then: Ensure the embedding is correct
    assert_eq!(embedding, vec![0.1, 0.2, 0.3]);

    // Cleanup
    env::remove_var("OPENAI_API_KEY")
}
