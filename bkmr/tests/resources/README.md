# Test Database Resources

This directory contains reference databases used for testing and development of the bkmr application.

## Database Files

### `schema_v1_migration_test.db`
- **Migration Level**: Single migration (20221229110455)
- **Content**: 11 sample bookmarks with basic schema
- **Features**: No embeddings, no file columns
- **Purpose**: Test migration path from old schema to current version
- **Used by**: `make run-migrate-db` target
- **Size**: ~53KB

### `schema_v2_with_embeddings.db` 
- **Migration Level**: Two migrations (up to 20231217200409)
- **Content**: 11 sample bookmarks with embedding data
- **Features**: Includes embedding BLOB data, FTS5 search table
- **Purpose**: Base test database for functionality requiring semantic search
- **Used by**: `make init` (copied to `../db/bkmr.db`), general testing
- **Size**: ~127KB

### `schema_v2_no_embeddings.db`
- **Migration Level**: Two migrations (up to 20231217200409) 
- **Content**: 11 sample bookmarks without embedding data
- **Features**: Schema supports embeddings but all embedding fields are NULL
- **Purpose**: Test functionality that should work without semantic search
- **Used by**: Tests that avoid embedding-dependent features
- **Size**: ~127KB

## Schema Evolution

The databases represent different points in the application's schema evolution:

1. **v1**: Basic bookmark storage with URL, metadata, tags, description
2. **v2**: Adds embedding support and full-text search capabilities  
3. **Current**: Adds created_ts, embeddable flag, and file import tracking

## Usage Patterns

### Development Workflow
```bash
make init          # Copies schema_v2_with_embeddings.db to ../db/bkmr.db
make test          # Auto-deletes and recreates clean test database
make run-migrate-db # Tests v1 → current migration path
```

### Testing Strategy
- **Unit tests**: Create fresh databases via `init_db()` migration system
- **Integration tests**: Use appropriate base database depending on features tested
- **Migration tests**: Start from v1 and verify upgrade to current schema

## Maintenance Notes

- These are the **single source of truth** for test data
- All databases contain identical bookmark content, only schema differs
- No duplicate copies should exist in `../db/` directory  
- When schema changes, regenerate these files from clean migrations
- Keep file sizes minimal for fast test execution

## Database Content Sample

All databases contain the same 11 test bookmarks:
- Google (id=1) - Basic search engine bookmark
- Various test entries with different tag combinations (aaa, bbb, ccc, xxx, yyy)
- Mix of URL types for comprehensive testing

The consistent content allows testing schema changes while maintaining predictable test data.