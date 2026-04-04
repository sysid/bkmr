pub(crate) mod connection;
pub(crate) mod error;
pub mod migration;
pub mod model;
pub mod repository;
pub mod schema;
pub mod vector_repository;

/// Register the sqlite-vec extension globally before any SQLite connection.
/// Must be called before opening any database connections.
/// Idempotent — safe to call multiple times.
pub fn register_sqlite_vec() {
    unsafe {
        rusqlite::ffi::sqlite3_auto_extension(Some(std::mem::transmute(
            sqlite_vec::sqlite3_vec_init as *const (),
        )));
    }
}
