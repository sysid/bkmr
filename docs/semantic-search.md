# Semantic Search with bkmr

`bkmr` offers powerful semantic search capabilities, allowing you to find relevant content based on meaning rather than just keywords. Semantic search runs **fully offline** using local embeddings — no API keys, no network calls, complete privacy.

## How It Works

Semantic search uses local embeddings (vector representations of text) via [fastembed](https://github.com/Anush008/fastembed-rs) (ONNX Runtime) to capture the meaning of your bookmarks and queries. Embeddings are stored in a `vec_bookmarks` virtual table powered by [sqlite-vec](https://github.com/asg017/sqlite-vec), enabling fast nearest-neighbor search directly in SQLite.

## Requirements

- No external API keys needed — everything runs locally
- First use downloads the embedding model (~500MB, one-time)
- Model cache location: `~/.cache/bkmr/models/` (override with `FASTEMBED_CACHE_DIR`)

## Basic Usage

```bash
# Semantic search for conceptually similar content
bkmr sem-search "containerized application security"

# Limit results to top 5 matches
bkmr sem-search "event-driven architecture" --limit 5

# Non-interactive mode
bkmr sem-search "microservice patterns" --np
```

## Integration with Smart Actions

Semantic search results work seamlessly with the action system. Each result will trigger the appropriate action based on its content type:

```bash
# Find and render documentation about Kubernetes
bkmr sem-search "kubernetes pod configuration"

# Find and execute shell scripts related to deployment
bkmr sem-search "deployment automation script"

# Find and copy code snippets for error handling
bkmr sem-search "error handling patterns"
```

## Embedding Defaults

By default, all new bookmarks are embeddable and participate in semantic search. Embeddings are generated automatically when you `add`, `load-json`, or `import-files`.

To opt out of embedding on creation:

```bash
# Add a bookmark without generating an embedding
bkmr add "https://example.com" tag1,tag2 --no-embed

# Import files without embeddings
bkmr import-files ./scripts --no-embed

# Bulk-load JSON without embeddings
bkmr load-json bookmarks.json --no-embed
```

## Managing Embeddings on Existing Bookmarks

Use `update --embed` / `--no-embed` to change the embeddable state of existing bookmarks:

```bash
# Enable embedding for an existing bookmark (generates embedding immediately)
bkmr update 123 --embed

# Disable embedding (removes embedding and content hash)
bkmr update 123 --no-embed

# Works with multiple IDs
bkmr update 123,456,789 --embed
```

**Note:** Bookmarks created before v7.6.0 default to non-embeddable. Use `update --embed` to enable them, then `backfill` to generate embeddings in batch.

## Batch Operations

```bash
# Backfill embeddings for all embeddable bookmarks that lack them
bkmr backfill

# Force regenerate all embeddings (clears embeddings + content hashes first)
bkmr backfill --force

# Preview what would be backfilled without making changes
bkmr backfill --dry-run

# Clear all embeddings and content hashes without regenerating
bkmr clear-embeddings
```

## Interactive Search Mode

When using semantic search without the `--np` flag, you'll get an interactive interface:

1. Results are displayed with their similarity scores
2. You can select which result(s) to open
3. The appropriate action will be executed based on content type

## Loading Text Documents for Semantic Search

You can import text documents to make them searchable via semantic search:

```bash
# Import text documents from a JSON file
bkmr load-texts path/to/documents.jsonl

# Preview importing without making changes
bkmr load-texts path/to/documents.jsonl --dry-run
```

The file should be in NDJSON format (one JSON object per line):

```json
{"id": "doc1.md", "content": "This is the content of document 1."}
{"id": "doc2.md", "content": "This is the content of document 2."}
```

## Markdown File Content Embedding

When working with markdown file references, `bkmr` can automatically embed the file content for semantic search when the file changes:

```bash
# Add a markdown file reference with embedding enabled
bkmr add "~/documents/research.md" research,notes --type md

# The content is automatically read, embedded, and a content hash is stored
```

When you access the bookmark later:
1. The file is read again
2. If the content has changed (detected via content hash), a new embedding is generated
3. The markdown is rendered with the updated content

This ensures your semantic search always uses the latest version of your documents without manual intervention.

## Developer Workflow Benefits

Semantic search transforms how developers access information:

1. **Concept-based retrieval** - Find information based on concepts, not just keywords
2. **Natural language queries** - Search the way you think, not how you tagged content
3. **Fully private** - All processing happens locally, nothing leaves your machine
4. **Action-ready results** - Results are immediately actionable based on content type
5. **Up-to-date content** - File content is automatically re-embedded when it changes

## Technical Details

- Default model: **NomicEmbedTextV15** (768 dimensions) — configurable via `config.toml`
- Uses asymmetric embeddings: `search_document:` prefix for storage, `search_query:` prefix for queries
- Embeddings stored in sqlite-vec `vec0` virtual table (`vec_bookmarks`)
- Nearest-neighbor search using Euclidean distance, converted to similarity score
- File content tracked using content hashes to minimize unnecessary re-embedding
- Model loaded lazily on first embed call — startup is fast

### Supported Models

Configure in `~/.config/bkmr/config.toml`:

```toml
[embeddings]
model = "NomicEmbedTextV15"  # default
```

| Model | Dimensions | Notes |
|-------|-----------|-------|
| `NomicEmbedTextV15` | 768 | Default, good general-purpose |
| `AllMiniLML6V2` | 384 | Smaller, faster |
| `BGESmallENV15` | 384 | Good for English |
| `BGEM3` | 1024 | Largest, most accurate |

Quantized variants (`*Q`) are also available for faster inference.

**After changing models**: Run `bkmr backfill --force` to regenerate all embeddings (this also clears content hashes). Alternatively, run `bkmr clear-embeddings` first for a clean slate, then `bkmr backfill`.

## Optimal Content for Embeddings

All bookmarks are embedded by default. Content that benefits most:

- Technical documentation and notes
- Complex code snippets with explanatory comments
- Project descriptions and requirements
- Reference materials and guides
- Markdown files that change frequently

Content where you might use `--no-embed`:
- Very short one-liner snippets
- URLs you only access by tag, not by meaning
- Temporary bookmarks you'll delete soon

## Privacy

Semantic search is **fully offline and private**:

- No API keys needed
- No network calls during embedding or search
- All processing happens locally via ONNX Runtime
- Model downloaded once and cached locally
- Your content never leaves your machine

## Combining with Template Interpolation

Semantic search works with template-enabled content but searches the template itself rather than rendered content. Keep this in mind when creating searchable templates.
