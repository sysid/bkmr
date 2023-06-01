use std::{env, process};
use clap::Parser;
use lazy_static::lazy_static;

// #[allow(dead_code)]
#[derive(Debug)]
pub struct Config {
    pub db_url: String,
    pub port: u16,
    pub fzf_opts: FzfEnvOpts
}

#[derive(Parser, Debug)]
pub struct FzfEnvOpts {
    #[clap(long, default_value = "50%")]
    pub height: String,
    #[clap(long, default_value_t = false)]
    pub reverse: bool,
    #[clap(long, default_value_t = false)]
    pub show_tags: bool,
}

impl Config {
    fn new() -> Config {
        let db_url = env::var("BKMR_DB_URL").unwrap_or_else(|_| "../db/bkmr.db".to_string());
        // test db_url as path exists
        let path = std::path::Path::new(&db_url);
        if !path.exists() {
            eprintln!("Error: db_url path does not exist: {:?}", db_url);
            process::exit(1);
        }
        let port = env::var("BKMR_PORT")
            .unwrap_or_else(|_| "9999".to_string())
            .parse()
            .expect("BKMR_PORT must be a number");

        let fzf_opts = env::var("BKMR_FZF_OPTS");

        /*
          clap::try_parse_from was first designed to parse
          a Vec containing the arguments of a basic shell command :
          the first item of the Vec must always be the command name.
          Nevertheless, if we have to parse an env variable like here, and not a shell command,
          we can easily insert an empty String to replace the command-name.
        */
        let fzf_opts_args = match &fzf_opts {
            Ok(options_string) => {
                let mut args = options_string.split(" ").collect::<Vec<_>>();
                args.insert(0, "");
                args
            },
            Err(_) => vec![""]
        };

        let Ok(fzf_opts) = FzfEnvOpts::try_parse_from(&fzf_opts_args) else {
            eprintln!("Error: Failed to parse BKMR_FZF_OPTS: {:?} \nPlease check bkmr documentation.", fzf_opts_args);
            process::exit(1)
        };

        Config { db_url, port, fzf_opts }
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
        println!("Using fzf defaults {:?}", CONFIG.fzf_opts);
        assert_eq!(CONFIG.port, 9999);
        assert_eq!(CONFIG.fzf_opts.height, String::from("100%"));
        assert_eq!(CONFIG.fzf_opts.reverse, true);
        assert_eq!(CONFIG.fzf_opts.show_tags, true);
    }
}
