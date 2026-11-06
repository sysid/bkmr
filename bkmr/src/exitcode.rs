/// Standard Unix exit codes for bkmr CLI application.
///
/// These codes follow the BSD convention where possible and provide
/// meaningful feedback about the type of error that occurred.
///
/// Successful termination
pub const SUCCESS: i32 = 0;

/// Runtime failure: the invocation was valid, but the operation failed
pub const FAILURE: i32 = 1;

/// Usage error - invalid arguments or an invalid config file; fix the invocation or config
pub const USAGE: i32 = 64;

/// Duplicate name conflict during import operation when --update flag is not provided
pub const DUP: i32 = 65;

/// Operation was cancelled by user at a confirmation prompt
pub const CANCEL: i32 = 130;
