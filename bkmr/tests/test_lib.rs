mod application;

use bkmr::util::testing::init_test_setup;

#[ctor::ctor]
fn init() {
    init_test_setup().expect("Failed to initialize test setup");
}
