# Advanced Usage Guide for bkmr

This document covers advanced usage patterns and techniques to maximize your productivity with bkmr.

## Tag Prefix Filtering

One of bkmr's most powerful features is its tag prefix filtering system, which allows you to create reusable search patterns.

### Understanding Tag Prefixes

Tag prefix options allow you to specify a set of tags that will be combined with command-line specified tags. This creates a union of the same tag types from both sources.

Available prefix options:
- `--tags-prefix`: Combined with `--tags` (all tags must match)
- `--ntags-prefix`: Combined with `--ntags` (any tag may match)
- `--Tags-prefix`: Combined with `--Tags` (none of these tags must match)
- `--Ntags-prefix`: Combined with `--Ntags` (none of these tags may match)

### Why Prefix Filtering is Powerful

Prefix filtering enables you to:

1. **Create specialized search contexts**: Pre-filter for certain content types
2. **Enforce content boundaries**: Automatically exclude certain content categories
3. **Simplify complex queries**: Combine multiple filtering strategies in shell functions
4. **Create domain-specific mini-applications**: Build custom tools for different use cases

## FTS Column Prefix Filtering

bkmr supports column-specific full-text search using the `column:term` syntax. When combined with prefix filtering, this becomes extremely powerful.

### Available FTS Columns

- `url:` - Search URLs only
- `metadata:` - Search metadata only (alias for title)
- `desc:` - Search descriptions only
- `tags:` - Search tags only

### Wildcard Matching

You can use `*` as a prefix search:
- `term*` - Terms starting with "term"

## Advanced Usage Examples

### Example 1: Smart Snippet Search

```bash
b() {
    bkmr search --fzf --fzf-style enhanced --Ntags-prefix _imported_ --tags-prefix _snip_ "metadata:${1}*"
}
```

This function:
1. Searches for snippets (includes tag prefix `_snip_`)
2. Excludes imported content (excludes tag prefix `_imported_`)
3. Searches only in titles starting with your search term (`metadata:${1}*`)
4. Uses enhanced FZF interface for better display

**Use case**: Finding code snippets by title prefix

```bash
# Find all Docker-related snippets
b docker

# Find all Git-related snippets
b git
```

### Example 2: Quick Snippet Creation

```bash
ba() {
    bkmr add -e -t snip "$@"
}
```

This function:
1. Creates a new snippet with `-e` (edit mode)
2. Automatically tags it with "snip"
3. Opens your editor to input the snippet content

**Use case**: Quickly save code snippets

```bash
# Create a new snippet (will open editor)
ba
```

### Example 3: Documentation-specific Searches

```bash
alias d-="BKMR_DB_URL=$HOME/vimwiki/buku/bm.db bkmr search --Ntags-prefix _snip_,_imported_ --tags-prefix doc"
alias d-aws="BKMR_DB_URL=$HOME/vimwiki/buku/bm.db bkmr search --fzf --Ntags-prefix _snip_,_imported_ --tags-prefix doc,aws"
```

These aliases:
1. Use specific URLs for documentation purposes
2. Exclude snippets and imported content (only URLs)
3. Include only items tagged with "doc"
4. For AWS docs, additionally filter for "aws" tag

**Use case**: Maintaining technology or language specific documentation sets

```bash
# Search all documentation, provide full search syntax
d-

# Search only AWS documentation with fuzzy finder
d-aws
```

## Creating Advanced Search Contexts

By combining prefix filtering with FTS column searches, you can create powerful search contexts for different needs:

### Project-specific References

```bash
proj-refs() {
    bkmr search --fzf --tags-prefix project,reference -t "$1" "$2"
}

# Usage: proj-refs [PROJECT-TAG] [OPTIONAL-SEARCH-TERM]
# Example: proj-refs frontend "react hooks"
```

## Advanced Filtering Techniques

### Combining Multiple Tag Types

You can combine different tag filtering methods:

```bash
# Find Python or Rust snippets that are NOT tagged as beginner
bkmr search --tags-prefix _snip_ -n python,rust -N beginner
```

### Complex FTS Queries

Full-text search supports complex queries:

```bash
# Find Docker entries with Compose or Swarm in the description
bkmr search "tags:docker desc:compose desc:swarm"
```

### Date-based Filtering with Sort Direction

Find recently added or the oldest entries:

```bash
# Most recently added entries
bkmr search --descending --limit 10

# Oldest entries that need review
bkmr search --ascending --tags needs-review
```

### Optimizing Tag Structure

Develop a consistent tagging strategy:

1. **Primary categories**: Use single-word tags like `python`, `docker`, `aws`
2. **Qualities/Properties**: Tags like `tutorial`, `reference`, `example`
3. **Projects**: Prefix with `project-` like `project-website`, `project-api`
4. **Status**: Tags like `active`, `archived`, `needs-review`

This structured approach makes prefix filtering even more powerful.

### Balancing Tag Specificity

Creating too many specific tags makes it harder to maintain consistency. Strike a balance:

- Too general: `code`, `document`
- Too specific: `python3.9-asyncio-example`, `aws-lambda-python-tutorial`
- Just right: `python`, `asyncio`, `aws`, `lambda`, `tutorial`

## Extending bkmr

### Integration with Other Tools

bkmr works well with other command-line tools:

```bash
# Use jq to process JSON output
bkmr search --json "python" | jq '.[] | {title, url}'

# Use fzf for additional filtering
bkmr search --json "programming" | jq -r '.[] | .title' | fzf
```

### Custom Output Processing

You can process search results for custom displays:

```bash
# Create a formatted HTML report of bookmarks
bkmr search --json "important" | jq -r '.[] | "<li><a href=\"\(.url)\">\(.title)</a></li>"' > bookmarks.html
```

### Backup and Version Control

Create automated backups of your bkmr databases:

```bash
# Daily backup script
backup-bkmr() {
    cp "$HOME/.config/bkmr/bkmr.db" "$HOME/backups/bkmr/bkmr-$(date +%Y%m%d).db"
    git -C "$HOME/backups/bkmr" add .
    git -C "$HOME/backups/bkmr" commit -m "Backup $(date +%Y-%m-%d)"
}
```

## Troubleshooting

### Common Issues

If your tag prefixes aren't working as expected:
- Verify your database contains the expected tags with `bkmr tags`

### Debugging Tips

Enable debug output to see what's happening:

```bash
bkmr -d search --tags-prefix project --ntags code
```

## Conclusion

By mastering tag prefix filtering and FTS column searches, you can transform bkmr from a simple bookmark manager into a powerful knowledge management system tailored to your specific workflows.

The combination of these features allows you to create specialized search contexts that feel like purpose-built tools, while maintaining a single source of truth for your data.
