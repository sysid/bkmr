#![allow(unused_imports)]
use log::*;
use stdext::function_name;

/// Logs debug information along with the current function name and line number.
///
/// This macro provides a way to quickly log messages while automatically
/// appending the function name and line number from where the log is called.
/// This can be beneficial in quickly identifying the source of a message
/// in complex codebases.
#[macro_export]
macro_rules! dlog {
    ($($arg:tt)*) => {{
        debug!("({}:{}) {}", file!(), function_name!(), format_args!($($arg)*));
    }}
}
#[macro_export]
macro_rules! dlog2 {
    ($($arg:tt)*) => {{
        debug!("({}:{}) {}", file!(), line!(), format_args!($($arg)*));
    }}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dlog_macro() {
        let test_var = vec![1, 2, 3];
        dlog!("Test variable: {:?}", &test_var);
        dlog!("Test variable: {:?}, {:?}", &test_var, "string");
        dlog2!("Test variable: {:?}, {:?}", &test_var, "string");
    }
}
