# LLM todo

# bkmr Refactoring and rsnip Integration Checklist

## Phase 1: Domain Layer Foundation

### Create Basic Domain Models
- [ ] Create `domain/mod.rs` - Main domain module
- [ ] Create `domain/bookmark.rs` - Bookmark entity
  - [ ] Extract core properties from existing implementation
  - [ ] Make it focused on domain behavior
  - [ ] Implement validation logic
  - [ ] Add methods for state changes
- [ ] Create `domain/tag.rs` - Tag value object
  - [ ] Implement proper validation
  - [ ] Add operations for tag manipulation
  - [ ] Create helper methods for tag normalization
- [ ] Create `domain/errors.rs` - Domain-specific errors
  - [ ] Define error types using thiserror
  - [ ] Add context information to errors
  - [ ] Implement Display and Debug traits

### Develop Domain Services
- [ ] Create `domain/services/mod.rs` - Services module
- [ ] Create `domain/services/bookmark_service.rs` - Service for bookmark operations
  - [ ] Define service trait with core operations
  - [ ] Implement service with bookmark manipulation logic
  - [ ] Add validation and business rules
- [ ] Create `domain/services/tag_service.rs` - Service for tag operations
  - [ ] Define service trait for tag operations
  - [ ] Implement methods for tag normalization and validation
  - [ ] Add operations for tag management (add/remove/merge)
- [ ] Create tests for domain services
  - [ ] Unit tests for bookmark service
  - [ ] Unit tests for tag service
  - [ ] Test for edge cases and error handling

### Implement Repository Interfaces
- [ ] Create `domain/repositories/mod.rs` - Repository module
- [ ] Create `domain/repositories/bookmark_repository.rs` - Interface for bookmark persistence
  - [ ] Define repository trait with CRUD operations
  - [ ] Add methods for searching and filtering
  - [ ] Define transaction support
- [ ] Create `domain/repositories/query.rs` - Query specifications
  - [ ] Implement base specification interface
  - [ ] Add common query specifications (ByTag, ByText, etc.)
  - [ ] Implement composition methods (And, Or, Not)
- [ ] Create tests for repository interfaces and specifications
  - [ ] Test query specification composition
  - [ ] Mock implementations for testing

## Phase 2: Application Layer Development

### Create Application Services
- [ ] Create `application/mod.rs` - Application module
- [ ] Create `application/services/mod.rs` - Services module
- [ ] Create `application/services/bookmark_application_service.rs` - Bookmark application service
  - [ ] Implement methods for bookmark operations
  - [ ] Add transaction coordination
  - [ ] Handle authorization and validation
- [ ] Create `application/services/tag_application_service.rs` - Tag application service
  - [ ] Implement methods for tag operations
  - [ ] Add bulk operations support
- [ ] Create `application/services/template_service.rs` - Template processing service
  - [ ] Add template rendering methods
  - [ ] Implement context management
- [ ] Add unit tests for application services
  - [ ] Test with mocked repositories
  - [ ] Verify correct behavior

### Implement DTOs and Mapping
- [ ] Create `application/dto/mod.rs` - DTO module
- [ ] Create `application/dto/bookmark_dto.rs` - Bookmark DTOs
  - [ ] Implement BookmarkCreateRequest
  - [ ] Implement BookmarkUpdateRequest
  - [ ] Implement BookmarkResponse
  - [ ] Add validation using serde
- [ ] Create `application/dto/tag_dto.rs` - Tag DTOs
  - [ ] Implement TagOperationRequest
  - [ ] Add validation for tag operations
- [ ] Create `application/dto/search_dto.rs` - Search DTOs
  - [ ] Implement BookmarkSearchRequest
  - [ ] Add pagination support
- [ ] Create mapping functions between DTOs and domain models
  - [ ] Implement from/into traits
  - [ ] Handle error cases

## Phase 3: Infrastructure Improvements

### Implement In-Memory Repository for Testing
- [ ] Create `infrastructure/repositories/mod.rs` - Repositories module
- [ ] Create `infrastructure/repositories/in_memory/mod.rs` - In-memory implementations
- [ ] Create `infrastructure/repositories/in_memory/bookmark_repository.rs` - In-memory repository
  - [ ] Implement in-memory storage with HashMaps
  - [ ] Add support for query specifications
  - [ ] Ensure thread safety
- [ ] Add tests for in-memory repository
  - [ ] Test CRUD operations
  - [ ] Test query specifications
  - [ ] Test concurrent access

### Refactor SQLite Repository
- [ ] Create `infrastructure/repositories/sqlite/mod.rs` - SQLite implementations
- [ ] Create `infrastructure/repositories/sqlite/bookmark_repository.rs` - SQLite repository
  - [ ] Refactor existing database code
  - [ ] Implement repository interface
  - [ ] Add transaction support
- [ ] Create `infrastructure/repositories/sqlite/connection.rs` - Connection management
  - [ ] Implement connection pooling
  - [ ] Add migration support
  - [ ] Handle database errors
- [ ] Create `infrastructure/repositories/sqlite/models.rs` - Database models
  - [ ] Define ORM models for database tables
  - [ ] Add mapping between domain and database models
- [ ] Add tests for SQLite repository
  - [ ] Integration tests with test database
  - [ ] Test migration process
  - [ ] Test error handling

### Implement Configuration Management
- [ ] Create `infrastructure/config/mod.rs` - Configuration module
- [ ] Create `infrastructure/config/settings.rs` - Application settings
  - [ ] Define strongly-typed configuration
  - [ ] Add validation
  - [ ] Set sensible defaults
- [ ] Create `infrastructure/config/provider.rs` - Configuration provider
  - [ ] Support multiple sources (env vars, files)
  - [ ] Implement override priorities
  - [ ] Add hot reloading support
- [ ] Add tests for configuration
  - [ ] Test loading from different sources
  - [ ] Test validation and error handling
  - [ ] Test defaults

## Phase 4: CLI Refactoring

### Refactor CLI Commands
- [ ] Create `cli/mod.rs` - CLI module
- [ ] Create `cli/commands/mod.rs` - Commands module
- [ ] Create `cli/commands/bookmark_commands.rs` - Bookmark commands
  - [ ] Implement search command
  - [ ] Implement add command
  - [ ] Implement update command
  - [ ] Implement delete command
- [ ] Create `cli/commands/tag_commands.rs` - Tag commands
  - [ ] Implement tag listing
  - [ ] Implement tag operations
- [ ] Add tests for CLI commands
  - [ ] Test command parsing
  - [ ] Test output formatting
  - [ ] Test error handling

### Enhance CLI Interaction
- [ ] Create `cli/interactive/mod.rs` - Interactive mode
- [ ] Create `cli/interactive/fuzzy_finder.rs` - Fuzzy finder UI
  - [ ] Implement interactive selection
  - [ ] Add preview capability
  - [ ] Support keyboard shortcuts
- [ ] Create `cli/output/mod.rs` - Output formatting
  - [ ] Implement table formatter
  - [ ] Add JSON output
  - [ ] Support colorized output
- [ ] Create `cli/input/mod.rs` - Input handling
  - [ ] Add interactive input
  - [ ] Implement validation
  - [ ] Support history
- [ ] Add tests for interactive features
  - [ ] Test fuzzy finding logic
  - [ ] Test UI rendering

## Phase 5: rsnip Integration Stage 1

### Add Template Support
- [ ] Create `domain/content/mod.rs` - Content module
- [ ] Create `domain/content/template.rs` - Template model
  - [ ] Define template structure
  - [ ] Add variables and expressions support
  - [ ] Support template inheritance
- [ ] Create `infrastructure/template/mod.rs` - Template module
- [ ] Create `infrastructure/template/engine.rs` - Template engine
  - [ ] Implement MiniJinja integration (from rsnip)
  - [ ] Add context management
  - [ ] Support filters and functions
- [ ] Create `infrastructure/template/safe_executor.rs` - Safe shell execution
  - [ ] Implement command validation
  - [ ] Add sandboxing for shell commands
  - [ ] Handle errors safely
- [ ] Add tests for template features
  - [ ] Test template rendering
  - [ ] Test variable substitution
  - [ ] Test error handling

### Implement Clipboard Management
- [ ] Create `infrastructure/clipboard/mod.rs` - Clipboard module
- [ ] Create `infrastructure/clipboard/provider.rs` - Clipboard provider
  - [ ] Implement cross-platform clipboard access
  - [ ] Add support for different content types
  - [ ] Handle platform-specific behavior
- [ ] Create `application/services/clipboard_service.rs` - Clipboard service
  - [ ] Add methods for copying bookmark content
  - [ ] Implement paste support
- [ ] Add tests for clipboard features
  - [ ] Test copy operations
  - [ ] Test paste operations
  - [ ] Test error handling

## Phase 6: rsnip Integration Stage 2

### Add Snippet Format Support
- [ ] Create `domain/parser/mod.rs` - Parser module
- [ ] Create `domain/parser/snippet_format.rs` - Snippet format enum
  - [ ] Define supported formats
  - [ ] Add format detection
- [ ] Create `infrastructure/parsers/mod.rs` - Parser implementations
- [ ] Create `infrastructure/parsers/default.rs` - Default format parser
  - [ ] Implement parsing logic
  - [ ] Add error handling
- [ ] Create `infrastructure/parsers/vcode.rs` - VS Code format parser
  - [ ] Implement VS Code snippet format support
  - [ ] Handle format-specific features
- [ ] Create `infrastructure/parsers/scls.rs` - SCLS format parser
  - [ ] Implement SCLS format support
  - [ ] Convert between formats
- [ ] Add tests for parsers
  - [ ] Test format detection
  - [ ] Test parsing logic
  - [ ] Test error handling

### Enhance Shell Integration
- [ ] Create `infrastructure/shell/mod.rs` - Shell module
- [ ] Create `infrastructure/shell/completion.rs` - Shell completion
  - [ ] Generate completion scripts for different shells
  - [ ] Support custom completions
- [ ] Create `infrastructure/shell/command.rs` - Command execution
  - [ ] Implement safe command execution
  - [ ] Add result processing
  - [ ] Handle errors
- [ ] Create `cli/commands/shell_commands.rs` - Shell integration commands
  - [ ] Add completion generation command
  - [ ] Implement shell helper commands
- [ ] Add tests for shell integration
  - [ ] Test completion script generation
  - [ ] Test command execution
  - [ ] Test error handling

## Phase 7: Final Integration

### Wire Everything Together
- [ ] Update `main.rs` to use new architecture
  - [ ] Initialize repositories
  - [ ] Set up application services
  - [ ] Configure dependency injection
- [ ] Create `app.rs` for application bootstrapping
  - [ ] Initialize all components
  - [ ] Set up logging
  - [ ] Handle startup errors
- [ ] Update existing code to work with new architecture
  - [ ] Adapt model.rs to new domain models
  - [ ] Update Dal to implement repository interfaces
  - [ ] Refactor service.rs to use application services
- [ ] Ensure backward compatibility
  - [ ] Add compatibility layer for existing code
  - [ ] Support legacy configuration
  - [ ] Maintain CLI compatibility

### Documentation and Testing
- [ ] Add comprehensive documentation
  - [ ] Update README.md with new features
  - [ ] Document architecture changes
  - [ ] Create migration guide for users
- [ ] Expand test coverage
  - [ ] Add end-to-end tests
  - [ ] Create benchmarks
  - [ ] Test all new features
- [ ] Perform code review
  - [ ] Check for consistency
  - [ ] Verify error handling
  - [ ] Ensure proper logging
- [ ] Optimize performance
  - [ ] Profile critical paths
  - [ ] Optimize database queries
  - [ ] Reduce memory usage

### Final Release Preparation
- [ ] Update version number
- [ ] Update CHANGELOG.md
- [ ] Prepare release notes
- [ ] Create migration scripts for database
- [ ] Test installation process
- [ ] Create example configurations

## Ongoing Tasks
- [ ] Regular code quality checks
- [ ] Performance monitoring
- [ ] Address technical debt
- [ ] Gather and incorporate user feedback
- [ ] Track compatibility with dependencies
