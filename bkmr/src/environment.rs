use std::{env, process};

use lazy_static::lazy_static;

// #[allow(dead_code)]
#[derive(Debug)]
pub struct Config {
    pub db_url: String,
    pub port: u16,
}

impl Config {
    fn new() -> Config {
        let db_url = env::var("BKMR_DB_URL").unwrap_or("../db/bkmr.db".to_string());
        // test db_url as path exists
        let path = std::path::Path::new(&db_url);
        if !path.exists() {
            eprintln!("Error: db_url path does not exist: {:?}", db_url);
            process::exit(1);
        }
        let port = env::var("PORT")
            .unwrap_or("9999".to_string())
            .parse()
            .expect("PORT must be a number");

        Config { db_url, port }
    }
}

// Create a global configuration singleton
lazy_static! {
    pub static ref CONFIG: Config = Config::new();
}

#[cfg(test)]
mod test {
    use rstest::*;

    use super::*;

    #[ctor::ctor]
    fn init() {
        let _ = env_logger::builder()
            // Include all events in tests
            .filter_level(log::LevelFilter::max())
            // Ensure events are captured by `cargo test`
            .is_test(true)
            // Ignore errors initializing the logger if tests race to configure it
            .try_init();
    }

    #[rstest]
    fn test_config() {
        println!("Using database at {}", CONFIG.db_url);
        println!("Listening on port {}", CONFIG.port);
        assert_eq!(CONFIG.db_url, String::from("../db/bkmr.db"));
        assert_eq!(CONFIG.port, 9999);
    }
}
