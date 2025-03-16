// bkmr/src/main.rs

use bkmr::cli;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bkmr::cli::setup_logging;
    use camino::Utf8PathBuf;
    use camino_tempfile::tempdir;
    use fs_extra::{copy_items, dir};
    use rstest::fixture;

    #[ctor::ctor]
    fn init() {
        setup_logging(2); // Set maximum debug level for tests
    }

    #[fixture]
    fn temp_dir() -> Utf8PathBuf {
        let tempdir = tempdir().unwrap();
        let options = dir::CopyOptions::new().overwrite(true);
        copy_items(
            &[
                "tests/resources/bkmr.v1.db",
                "tests/resources/bkmr.v2.db",
                "tests/resources/bkmr.v2.noembed.db",
            ],
            "../db",
            &options,
        )
        .expect("Failed to copy test project directory");

        tempdir.into_path()
    }

    #[test]
    fn verify_cli() {
        use clap::CommandFactory;
        Cli::command().debug_assert()
    }
}
