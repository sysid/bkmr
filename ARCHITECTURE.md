# Architectural Guidelines: Use of `Arc<dyn Trait>` Pattern

## Core Principle

In the BKMR codebase, we follow a consistent pattern of wrapping service and repository implementations in `Arc<dyn Trait>` to enable flexible dependency injection while maintaining thread-safe shared ownership.

## Pattern Definition

1. **Services**: All service implementations (e.g., `BookmarkServiceImpl`, `ActionServiceImpl`) are:
   - Created by factory functions that return `Arc<dyn ServiceTrait>`
   - Never exposed as concrete types to consumers
   - Always passed around as `Arc<dyn ServiceTrait>`

2. **Repositories**: All repository implementations (e.g., `SqliteBookmarkRepository`) are:
   - Created once and wrapped in `Arc<dyn RepositoryTrait>`
   - Shared across multiple services
   - Accessed via reference-counted pointers

3. **Cross-Layer Dependencies**: When services depend on other services or repositories:
   - They always store an `Arc<dyn Trait>` field
   - They accept `Arc<dyn Trait>` in constructors
   - They never unwrap or store concrete types

## Architectural Consequences

1. **Dependency Injection**: This pattern enables a simple form of dependency injection where:
   - Factory functions create and wire together components
   - The `AppState` singleton provides global access to core services
   - Testing can substitute mock implementations via trait objects

2. **Thread Safety**: All services and repositories can be safely shared across threads because:
   - `Arc` provides thread-safe reference counting
   - Trait objects hide implementation details
   - Internal mutability is handled via repository implementations

3. **Service Lifetime**: Services and repositories exist for the application's entire lifetime:
   - They are created once at startup
   - They may be cloned but never dropped until shutdown
   - No ownership conflicts occur due to reference counting

4. **Polymorphism**: The system can swap implementations without changing consumer code:
   - Different repository backends (SQLite, in-memory, etc.)
   - Alternative service implementations for different contexts
   - Mocks for testing

## Implementation Rules

1. **Factory Functions**:
   ```rust
   // Always return Arc<dyn Trait>, never concrete types
   pub fn create_bookmark_service() -> Arc<dyn BookmarkService> {
       // Implementation details here
       Arc::new(BookmarkServiceImpl::new(/* dependencies */))
   }
   ```

2. **Service Constructors**:
   ```rust
   pub struct BookmarkServiceImpl<R: BookmarkRepository> {
       // Always store dependencies as Arc<dyn Trait>
       repository: Arc<R>,
       embedder: Arc<dyn Embedder>,
       import_repository: Arc<dyn ImportRepository>,
   }

   impl<R: BookmarkRepository> BookmarkServiceImpl<R> {
       // Always accept Arc<dyn Trait> for dependencies
       pub fn new(
           repository: Arc<R>,
           embedder: Arc<dyn Embedder>,
           import_repository: Arc<dyn ImportRepository>,
       ) -> Self {
           Self { repository, embedder, import_repository }
       }
   }
   ```

3. **Consumers**:
   ```rust
   // Accept services as Arc<dyn Trait>
   fn process_bookmarks(service: Arc<dyn BookmarkService>) {
       // Use service methods directly
       let bookmarks = service.get_all_bookmarks(None, None)?;
       
       // Can clone for async operations or parallel processing
       let service_clone = service.clone();
       tokio::spawn(async move {
           service_clone.process_async().await;
       });
   }
   ```

## When to Use Other Patterns

1. **Non-Shared Components**: For components used only in a single context and not shared:
   - Consider `Box<dyn Trait>` if dynamic dispatch is needed but not sharing
   - Consider concrete types if polymorphism isn't required

2. **Function-Local Dependencies**: For dependencies used only within a single function:
   - Consider passing by reference (`&dyn Trait`) instead of Arc
   - Use concrete types when possible

3. **Short-lived Objects**: For objects with clear lifetimes:
   - Domain entities generally shouldn't use Arc
   - Values returned from service methods may use simpler ownership patterns

## Conclusion

The consistent use of `Arc<dyn Trait>` throughout the service and repository layers provides a predictable, 
flexible architecture that supports dependency injection, polymorphism, and thread safety.

This pattern should be followed for all new services and repositories to maintain architectural consistency.