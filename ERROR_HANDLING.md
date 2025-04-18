## The Role of `SqliteRepositoryError`

1. **Infrastructure-Specific Details**: It captures SQLite-specific error cases that the domain layer shouldn't need to know about (connection issues, query failures, etc.)

2. **Error Context**: It can add SQLite-specific context to errors before they're converted to domain errors

3. **Error Isolation**: It keeps infrastructure implementation details isolated from your domain model

## Error Conversion Flow

```
Infrastructure Errors → Domain Errors → Application Errors → Presentation Errors
```

For `SqliteRepositoryError`, the flow should be:

1. **Low-level failures** (Diesel errors, connection errors) → `SqliteRepositoryError`
2. `SqliteRepositoryError` → `DomainError` (when crossing to domain layer)
3. `DomainError` → `ApplicationError` (when crossing to application layer)
4. `ApplicationError` → `CliError` (when presenting to the user)

## Implementation Example

```rust
// Converting SqliteRepositoryError to DomainError
impl From<SqliteRepositoryError> for DomainError {
    fn from(err: SqliteRepositoryError) -> Self {
        match err {
            SqliteRepositoryError::BookmarkNotFound(id) => {
                DomainError::BookmarkNotFound(id.to_string())
            }
            _ => DomainError::BookmarkOperationFailed(err.to_string()),
        }
    }
}

// Converting DomainError to ApplicationError
impl From<DomainError> for ApplicationError {
    fn from(err: DomainError) -> Self {
        ApplicationError::Domain(err)
    }
}
```

## Best Practices

1. **Keep Infrastructure Errors**: `SqliteRepositoryError` for infrastructure-specific error details

2. **Explicit Conversions**: `From` implementations for converting between error types

3. **Error Mapping in Repositories**: map SQLite errors to domain errors when crossing boundary:

```rust
fn get_by_id(&self, id: i32) -> Result<Option<Bookmark>, DomainError> {
    self.get_connection()
        .map_err(DomainError::from)?  // SqliteRepositoryError → DomainError
        .find_by_id(id)
        .map_err(|e| DomainError::from(e))?  // SqliteRepositoryError → DomainError
}
```

4. **Error Enrichment**: Add context when converting errors:

```rust
// Better error conversion with context
fn from(err: SqliteRepositoryError) -> DomainError {
    match err {
        SqliteRepositoryError::ConnectionError(msg) => 
            DomainError::BookmarkOperationFailed(format!("Database connection failed: {}", msg)),
        // Other cases...
    }
}
```

## Summary

1. **Repository Layer**: Convert Diesel/database errors to `SqliteRepositoryError`
2. **Domain Boundary**: Convert `SqliteRepositoryError` to `DomainError`  
3. **Application Boundary**: Convert `DomainError` to `ApplicationError`
4. **Presentation Boundary**: Convert `ApplicationError` to `CliError` or other presentation errors
