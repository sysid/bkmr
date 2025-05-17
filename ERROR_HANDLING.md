# Error Handling Guidelines for BKMR Project

## Error Type Hierarchy

```
Infrastructure → Domain → Application → Presentation (CLI)
```

### Key Error Types

1. **Infrastructure Layer**
   - `SqliteRepositoryError`: Database-specific errors
   - `InfrastructureError`: General infrastructure failures
   - `InterpolationError`: Template rendering failures

2. **Domain Layer**
   - `DomainError`: Core business logic errors
   - `RepositoryError`: Abstract repository interface errors

3. **Application Layer**
   - `ApplicationError`: Service-level errors

4. **Presentation Layer**
   - `CliError`: Command-line interface errors

## Core Error Handling Principles

1. **Error Context Enhancement**
   - All error types implement a `context()` method for adding context
   - Context should clarify where and why the error occurred

   ```rust
   // Example: Adding context to an error
   repository.get_by_id(id)
       .map_err(|e| e.context(format!("Failed to retrieve bookmark {}", id)))?
   ```

2. **Explicit Error Conversion**
   - Use `From` traits to convert between error types at layer boundaries
   - More specific conversions for important cases, fallback for others

   ```rust
   // Example: Converting SqliteRepositoryError to DomainError
   impl From<SqliteRepositoryError> for DomainError {
       fn from(err: SqliteRepositoryError) -> Self {
           match err {
               SqliteRepositoryError::BookmarkNotFound(id) => 
                   DomainError::BookmarkNotFound(id.to_string()),
               SqliteRepositoryError::DatabaseError(e) => 
                   DomainError::RepositoryError(RepositoryError::Database(e.to_string())),
               // Other specific mappings...
               _ => DomainError::RepositoryError(RepositoryError::Other(err.to_string())),
           }
       }
   }
   ```

3. **Error Propagation with `?` Operator**
   - Use the `?` operator for clean error propagation
   - Convert error types at layer boundaries with `.map_err()`

   ```rust
   fn add_bookmark(&self, ...) -> ApplicationResult<Bookmark> {
       // Convert domain errors to application errors implicitly with ?
       let bookmark = self.repository.add(&mut bookmark)?;
       
       // Or explicitly with map_err
       self.repository.add(&mut bookmark)
           .map_err(|e| ApplicationError::Domain(e))?;
   }
   ```

4. **Error Type Patterns**
   - Use `thiserror` for defining structured error enums
   - Include helpful error messages in the `#[error("...")]` attributes
   - Provide serialized field values in error messages

   ```rust
   #[derive(Error, Debug)]
   pub enum DomainError {
       #[error("Bookmark not found: {0}")]
       BookmarkNotFound(String),
       
       #[error("Repository error: {0}")]
       RepositoryError(#[from] RepositoryError),
       
       // Other error variants...
   }
   ```

5. **Result Type Aliases**
   - Define `Result` type aliases for each layer

   ```rust
   pub type DomainResult<T> = Result<T, DomainError>;
   pub type ApplicationResult<T> = Result<T, ApplicationError>;
   pub type CliResult<T> = Result<T, CliError>;
   pub type SqliteResult<T> = Result<T, SqliteRepositoryError>;
   ```

## Error Presentation

- CLI errors should be user-friendly with actionable information
- Use color in terminal output (via `crossterm` crate) to highlight errors
- Include suggestions for resolution when possible

```rust
eprintln!("{}", "Error: Database not found.".red());
eprintln!("Either:");
eprintln!("  1. Set BKMR_DB_URL environment variable to point to an existing database");
eprintln!("  2. Create a database using 'bkmr create-db <path>'");
```