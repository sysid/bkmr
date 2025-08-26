/// Standard Unix exit codes for bkmr CLI application.
///
/// These codes follow the BSD convention where possible and provide
/// meaningful feedback about the type of error that occurred.
///
/// Successful termination
pub const SUCCESS: i32 = 0;

/// Command line usage error - invalid arguments, missing required parameters, etc.
pub const USAGE: i32 = 64;

/// Duplicate name conflict during import operation when --update flag is not provided
pub const DUP: i32 = 65;

/// Operation was cancelled by user (typically Ctrl+C)
pub const CANCEL: i32 = 130;
