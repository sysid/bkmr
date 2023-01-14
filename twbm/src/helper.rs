use diesel::sqlite::Sqlite;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};
use log::debug;
use std::error::Error;
use camino::Utf8Path;
use stdext::function_name;

pub fn init_logger() {
    let _ = env_logger::builder()
        // Include all events in tests
        .filter_level(log::LevelFilter::max())
        // Ensure events are captured by `cargo test`
        .is_test(true)
        // Ignore errors initializing the logger if tests race to configure it
        .try_init();
}

pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

#[allow(unused)]
pub fn init_db(
    connection: &mut impl MigrationHarness<Sqlite>,
) -> Result<(), Box<dyn Error + Send + Sync + 'static>> {
    debug!("({}:{}) {:?}", function_name!(), line!(), "--> initdb <--");
    connection.revert_all_migrations(MIGRATIONS)?;
    connection.run_pending_migrations(MIGRATIONS)?;
    Ok(())
}

pub fn ensure_int_vector(vec: &Vec<String>) -> Option<Vec<i32>> {
    vec.iter()
        .map(|s| s.parse::<i32>())
        .collect::<Result<Vec<_>, _>>()
        .map(|mut v| {
            v.sort_by(|a, b| a.cmp(b));
            v
        }).ok()
}

/// resolves existing path and follows symlinks, returns None if path does not exist
pub fn abspath(p: &str) -> Option<String> {
    let abs_p = shellexpand::full(p)
        .ok()
        .and_then(|x| Utf8Path::new(x.as_ref()).canonicalize_utf8().ok())
        .and_then(|p| Some(p.into_string()));
    debug!("({}:{}) {:?} -> {:?}", function_name!(), line!(), p, abs_p);
    abs_p
}

#[cfg(test)]
mod test {
    // use log::debug;
    use super::*;
    use rstest::*;

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

    // todo: emtpy vec
    #[rstest]
    #[case(vec!["1".to_string(), "2".to_string(), "3".to_string()], Some(vec![1, 2, 3]))]
    #[case(vec!["3".to_string(), "1".to_string(), "2".to_string()], Some(vec![1, 2, 3]))]
    #[case(vec!["a".to_string(), "2".to_string(), "3".to_string()], None)]
    fn test_ensure_int_vector(#[case] x: Vec<String>, #[case] expected: Option<Vec<i32>>) {
        assert_eq!(ensure_int_vector(&x), expected);
    }

    // Tests are fragile, because they depend on machine specific setup
    #[rstest]
    // #[case("", None)]
    // #[case("~/dev/binx", Some("/Users/Q187392/dev/s/private/devops-binx".to_string()))]  // link resolved
    // #[case("$HOME/dev/binx", Some("/Users/Q187392/dev/s/private/devops-binx".to_string()))]
    // #[case("https://www.google.com", None)]
    #[case("./tests/resources/rtwbm.pptx", Some("/Users/Q187392/dev/s/private/rs-twbm/twbm/tests/resources/rtwbm.pptx".to_string()))]  // link resolved
    fn test_abspath(#[case] x: &str, #[case] expected: Option<String>) {
        assert_eq!(abspath(x), expected);
    }
}
