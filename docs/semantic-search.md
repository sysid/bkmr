# Semantic Search with bkmr

`bkmr` offers powerful semantic search capabilities, allowing you to find relevant content based on meaning rather than just keywords.

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

## Technical Details

- `bkmr` uses OpenAI's text-embedding-ada-002 model by default
- Only portions of bookmarks marked as embeddable are sent to OpenAI for embedding generation
- Embeddings and content hashes are stored locally in your database
- Similarity is calculated using cosine similarity between vector representations

## Privacy Considerations

When using the OpenAI integration:

- Content from your bookmarks is sent to OpenAI's API for embedding generation
- No content is stored by OpenAI, but it may be used to improve their services
- If you have privacy concerns, consider carefully which bookmarks you mark as embeddable