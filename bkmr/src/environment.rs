use crate::dlog2;
use clap::Parser;
use lazy_static::lazy_static;
use log::debug;
use std::{env, process};

// #[allow(dead_code)]
#[derive(Debug)]
pub struct Config {
    pub db_url: String,
    pub fzf_opts: FzfEnvOpts,
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
                let mut args = options_string.split(' ').collect::<Vec<_>>();
                args.insert(0, "");
                args
            }
            Err(_) => vec![""],
        };

        let Ok(fzf_opts) = FzfEnvOpts::try_parse_from(&fzf_opts_args) else {
            eprintln!(
                "Error: Failed to parse BKMR_FZF_OPTS: {:?} \nPlease check bkmr documentation.",
                fzf_opts_args
            );
            process::exit(1)
        };
        dlog2!("db: {:?}, fzf_opts: {:?}", db_url, fzf_opts);

        Config { db_url, fzf_opts }
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
        println!("Using fzf defaults {:?}", CONFIG.fzf_opts);
        assert_eq!(CONFIG.fzf_opts.height, String::from("100%"));
        assert!(CONFIG.fzf_opts.reverse);
        assert!(CONFIG.fzf_opts.show_tags);
    }
}
