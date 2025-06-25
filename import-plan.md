# Import Function Implementation Plan

## Overview
Implement `bkmr import DIR... [--update] [--delete-missing]` to recursively scan directories for `.sh`, `.py`, `.md` files, parse frontmatter metadata, and create/update bkmr bookmarks while maintaining existing architectural patterns.

## Requirements
- Sub-command: `bkmr import DIR… [--update] [--delete-missing]`
- Recursively scan DIR arguments for files ending in `.sh`, `.py`, `.md`
- Parse YAML-or-hash-style front-matter: `name` (mandatory, unique), `tags`, `type` (default `_shell_`)
- Compute SHA-256 of content
- INSERT INTO bookmarks; on conflict(name) DO UPDATE when --update present and content_hash differs
- When --delete-missing, DELETE bookmarks whose src_path no longer exists under supplied DIR roots
- Print summary: "Added X Updated Y Deleted Z"
- Exit with code 65 when duplicate name found and --update absent

## Implementation Status

### ✅ Completed Components

#### 1. Exit Code System
- **File**: `src/exitcode.rs`
- **Constants**: SUCCESS (0), USAGE (64), DUP (65), CANCEL (130)
- **Integration**: Updated `main.rs` to use constants instead of bare numbers
- **Documentation**: Added exit code table to `ERROR_HANDLING.md`

#### 2. Domain Layer
- **File**: `src/domain/repositories/import_repository.rs`
- **Types**:
  - `FileImportData`: Holds parsed file metadata (name, tags, type, content, file_path, file_mtime, file_hash)
  - `ImportOptions`: Configuration struct (update, delete_missing, dry_run)
- **Trait Extension**: Added `import_files()` method to `ImportRepository` trait

#### 3. Infrastructure Layer
- **File**: `src/infrastructure/repositories/file_import_repository.rs`
- **Implementation**: Complete `FileImportRepository` with:
  - Directory traversal using `ignore::WalkBuilder` (respects .gitignore)
  - Frontmatter parsing for both YAML and hash-style comments
  - Content hashing using SHA-256
  - File metadata extraction (mtime, path)
  - Comprehensive error handling and logging
- **Dependencies Added**: `serde_yaml`, `sha2`, `ignore` to Cargo.toml
- **File Type Support**: `.sh`, `.py`, `.md` files only
- **Frontmatter Formats**:
  ```yaml
  ---
  name: backup-database
  tags: admin, backup, database
  type: _shell_
  ---
  ```
  ```bash
  #!/bin/bash
  # name: backup-database
  # tags: admin, backup, database
  # type: _shell_
  ```

#### 4. Application Layer (Partial)
- **File**: `src/application/services/bookmark_service.rs`
- **Method Added**: `import_files(&self, paths: &[String], update: bool, delete_missing: bool, dry_run: bool) -> ApplicationResult<(usize, usize, usize)>`
- **Return Type**: Tuple of (added_count, updated_count, deleted_count)

### 🔄 In Progress

#### 5. Service Implementation
- **File**: `src/application/services/bookmark_service_impl.rs`
- **Method**: `import_files()` implementation
- **Required Logic**:
  1. **File Discovery**: Use `FileImportRepository` to scan directories
  2. **Duplicate Detection**: Check existing bookmarks by name (unique constraint)
  3. **Conflict Resolution**:
     - If duplicate found and `--update` not present: Exit with code 65
     - If duplicate found and `--update` present: Compare SHA-256 hashes
     - Only update if content hash differs
  4. **Create/Update Operations**:
     - Convert `FileImportData` to `Bookmark` domain objects
     - Set appropriate system tags based on file type
     - Update file metadata fields (file_path, file_mtime, file_hash)
  5. **Delete Missing**: When `--delete-missing` flag set:
     - Find bookmarks with file_path set but source file no longer exists
     - Delete orphaned bookmarks
  6. **Summary Reporting**: Return counts for added/updated/deleted bookmarks

### ⏳ Pending Components

#### 6. CLI Integration
- **File**: `src/cli/args.rs`
- **Command**: Add `ImportFiles` variant to `Commands` enum
- **Arguments**: 
  ```rust
  ImportFiles {
      #[arg(help = "Directories or files to import")]
      paths: Vec<String>,
      #[arg(short = 'u', long = "update", help = "Update existing bookmarks when content differs")]
      update: bool,
      #[arg(long = "delete-missing", help = "Delete bookmarks whose source files no longer exist")]
      delete_missing: bool,
      #[arg(short = 'd', long = "dry-run", help = "Show what would be done without making changes")]
      dry_run: bool,
  }
  ```

#### 7. Command Handler
- **File**: `src/cli/bookmark_commands.rs`
- **Function**: `import_files()` handler
- **Features**:
  - Progress reporting during directory scan
  - Dry-run mode with detailed preview
  - Colored summary output using existing patterns
  - Proper exit code handling (especially exit 65 for duplicates)

#### 8. Service Factory Integration
- **File**: `src/application/services/factory.rs`
- **Function**: `create_file_import_repository()` factory
- **Integration**: Wire FileImportRepository into service dependency injection

#### 9. Testing Strategy
- **Unit Tests**:
  - FileImportRepository: file parsing, frontmatter extraction, error handling
  - BookmarkService: import logic, conflict resolution, update/delete behavior
  - CLI: argument parsing, dry-run mode, summary output
- **Integration Tests**:
  - End-to-end import scenarios with temporary directories
  - File modification detection and update behavior
  - Delete missing functionality
  - Error recovery and validation
- **Test Data Structure**:
  ```
  tests/resources/import_test/
  ├── scripts/
  │   ├── backup.sh (with YAML frontmatter)
  │   └── deploy.py (with hash comments)
  ├── docs/
  │   └── readme.md (with YAML frontmatter)
  └── invalid/
      └── malformed.sh (invalid frontmatter)
  ```

## Database Schema Integration

The import function leverages the newly added database columns:
- `file_path` (TEXT NULL): Source file path for tracking
- `file_mtime` (INTEGER NULL): File modification time (Unix timestamp)  
- `file_hash` (TEXT NULL): SHA-256 hash of file content

These fields enable:
- **Change Detection**: Compare current file hash with stored hash
- **Orphan Detection**: Find bookmarks whose source files no longer exist
- **Update Optimization**: Only update bookmarks when content actually changes

## Error Handling Strategy

### Exit Codes
- **0 (SUCCESS)**: All files imported successfully, or no conflicts found
- **64 (USAGE)**: Invalid command line arguments or configuration errors
- **65 (DUP)**: Duplicate name found without --update flag (stops on first duplicate)
- **130 (CANCEL)**: User cancelled operation (Ctrl+C)

### Duplicate Name Handling
When duplicate name found without --update:
```
Error: Duplicate name 'backup-script' found in /path/to/script.sh
Existing bookmark with same name already exists (ID: 42)
Use --update flag to overwrite existing bookmarks with changed content
```

### Error Recovery
- Invalid frontmatter: Skip file with warning, continue processing
- Missing mandatory name: Skip file with error, continue processing
- File access errors: Skip file with warning, continue processing
- Hash computation errors: Skip file with error, continue processing

## Implementation Order
1. ✅ **Domain types and traits** - Core abstractions
2. ✅ **Infrastructure repository** - File parsing and directory scanning  
3. ✅ **Application service interface** - Service trait extension
4. 🔄 **Service implementation** - Business logic integration
5. ⏳ **CLI command** - User interface
6. ⏳ **Testing** - Comprehensive test coverage
7. ⏳ **Documentation** - Usage examples and patterns

## Architecture Compliance

The implementation follows existing bkmr architectural patterns:
- **Clean Architecture**: Clear separation between domain, application, infrastructure, and CLI layers
- **Repository Pattern**: File import abstracted behind `ImportRepository` trait
- **Service Pattern**: Business logic orchestrated in `BookmarkService`
- **Factory Pattern**: Dependency injection with `Arc<dyn Trait>` pattern
- **Error Handling**: Structured error hierarchy with `thiserror` and context preservation
- **CLI Patterns**: Consistent command structure with `clap` derive macros

## Next Steps

1. **Complete Service Implementation**: Implement `import_files()` in `BookmarkServiceImpl`
   - Focus on duplicate name detection with proper exit code 65
   - Implement content change detection via SHA-256 comparison
   - Add delete missing functionality

2. **CLI Integration**: Add command variant and handler
   - Ensure proper exit code propagation to main.rs
   - Implement progress reporting and colored output

3. **Comprehensive Testing**: Create test scenarios covering all edge cases
   - Duplicate handling with and without --update
   - File modification detection
   - Delete missing functionality