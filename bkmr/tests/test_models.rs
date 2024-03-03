// #![allow(unused_imports, unused_variables)]

use std::env;

use bkmr::adapter::embeddings::{Context, OpenAi};
use bkmr::CTX;
use rstest::*;

use bkmr::helper::calc_content_hash;
use bkmr::model::bookmark::{BookmarkBuilder, BookmarkUpdater};

#[ctor::ctor]
fn init() {
    env::set_var("SKIM_LOG", "info");
    env::set_var("TUIKIT_LOG", "info");
    let _ = env_logger::builder()
        // Include all events in tests
        .filter_level(log::LevelFilter::max())
        .filter_module("skim", log::LevelFilter::Info)
        .filter_module("tuikit", log::LevelFilter::Info)
        .filter_module("mio", log::LevelFilter::Info)
        .filter_module("reqwest", log::LevelFilter::Info)
        // Ensure events are captured by `cargo test`
        .is_test(true)
        // Ignore errors initializing the logger if tests race to configure it
        .try_init();
}

#[rstest]
fn test_bm_update() {
    // Given: Request a new server from the pool
    let mut server = mockito::Server::new();
    let url = server.url();
    env::set_var("OPENAI_API_KEY", "test_key");
    let mock = server
        .mock("POST", "/v1/embeddings")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"data": [{"embedding": [0.1, 0.2, 0.3]}]}"#)
        .expect(2)
        .create();
    // Given: OpenAi strategy/context with mocked url/server
    let open_ai = OpenAi::new(url);
    CTX.set(Context::new(Box::new(open_ai))).unwrap();

    // When: new bm created without update()
    let mut bm = BookmarkBuilder::new()
        .id(1)
        .URL("www.sysid.de".to_string())
        .metadata("metadata".to_string())
        .tags(",aaa,xxx,".to_string())
        .desc("desc".to_string())
        .flags(0)
        .build();
    println!("{:?}", bm);
    assert_eq!(bm.id, 1);
    // Then: embedding is None / content_hash is Some
    let expected_hash = calc_content_hash(",aaa,xxx,metadata -- desc,aaa,xxx,");
    assert_eq!(bm.content_hash, Some(expected_hash.clone()));
    assert!(bm.embedding.is_none());

    // When: update() is called first time
    bm.update();
    // Then: embedding is Some, content_hash has not changed
    assert_eq!(bm.content_hash, Some(expected_hash.clone()));
    assert!(bm.embedding.is_some());
    println!("{:?}", bm);

    // When: update() is called second time with no changes
    bm.update();
    // Then: embedding is Some, content_hash has not changed, no request
    assert_eq!(bm.content_hash, Some(expected_hash.clone()));

    // When: update() is called third time with changes
    bm.metadata = "changed".to_string();
    // Then: embedding is Some, content_hash has changed, new request
    let expected_hash = calc_content_hash(",aaa,xxx,changed -- desc,aaa,xxx,");
    bm.update();
    assert_eq!(bm.content_hash, Some(expected_hash.clone()));
    println!("{:?}", bm);

    // Ensure all expectations have been met, i.e. 2 requests
    mock.assert();

    // Cleanup
    env::remove_var("OPENAI_API_KEY")
}
