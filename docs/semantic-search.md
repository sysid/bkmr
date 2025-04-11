# Semantic Search with bkmr

`bkmr` offers powerful semantic search capabilities, allowing you to find relevant content based on meaning rather than just keywords. This AI-powered feature helps developers locate information even when they don't remember the exact terms or tags.

## How It Works

Semantic search uses AI embeddings (vector representations of text) to capture the meaning of your bookmarks and queries. This allows `bkmr` to find content that's conceptually related, even when it doesn't contain the exact search terms.

## Requirements

- OpenAI API key set as environment variable: `OPENAI_API_KEY`
- The `--openai` flag when running commands that use embeddings

## Basic Usage

```bash
# Enable OpenAI embeddings and search for conceptually similar content
bkmr --openai sem-search "containerized application security"

# Limit results to top 5 matches
bkmr --openai sem-search "event-driven architecture" --limit 5

# Non-interactive mode
bkmr --openai sem-search "microservice patterns" --np
```

## Integration with Smart Actions

Semantic search results work seamlessly with the action system. Each result will trigger the appropriate action based on its content type:

```bash
# Find and render documentation about Kubernetes
bkmr --openai sem-search "kubernetes pod configuration"

# Find and execute shell scripts related to deployment
bkmr --openai sem-search "deployment automation script"

# Find and copy code snippets for error handling
bkmr --openai sem-search "error handling patterns"
```

## Managing Embeddable Content

Not all content benefits from semantic embeddings. By default, new bookmarks are not marked as embeddable to save API costs.

```bash
# Mark a bookmark as embeddable (will generate embeddings)
bkmr set-embeddable 123 --enable

# Mark a bookmark as non-embeddable 
bkmr set-embeddable 123 --disable

# Backfill embeddings for all embeddable bookmarks
bkmr --openai backfill

# Preview what would be backfilled without making changes
bkmr --openai backfill --dry-run
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
bkmr --openai load-texts path/to/documents.jsonl

# Preview importing without making changes
bkmr --openai load-texts path/to/documents.jsonl --dry-run
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
bkmr --openai add "~/documents/research.md" research,notes --type md

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
3. **Comprehensive knowledge base** - Build a personal AI-powered documentation system
4. **Action-ready results** - Results are immediately actionable based on content type
5. **Up-to-date content** - File content is automatically re-embedded when it changes

## Technical Details

- `bkmr` uses OpenAI's text-embedding-ada-002 model by default
- Only portions of bookmarks marked as embeddable are sent to OpenAI for embedding generation
- Embeddings and content hashes are stored locally in your database
- Similarity is calculated using cosine similarity between vector representations
- File content is tracked using content hashes to minimize unnecessary API calls

## Optimal Content for Embeddings

Not all content types benefit equally from embeddings. Consider enabling embeddings for:

- Technical documentation and notes
- Complex code snippets with explanatory comments
- Project descriptions and requirements
- Reference materials and guides
- Markdown files that change frequently

Content that may not benefit as much:
- Very short snippets or one-liners
- URLs without descriptive content
- Binary files or executables

## Privacy Considerations

When using the OpenAI integration:

- Content from your bookmarks is sent to OpenAI's API for embedding generation
- No content is stored by OpenAI, but it may be used to improve their services
- If you have privacy concerns, consider carefully which bookmarks you mark as embeddable

## Combining with Template Interpolation

Semantic search works with template-enabled content but searches the template itself rather than rendered content. Keep this in mind when creating searchable templates.