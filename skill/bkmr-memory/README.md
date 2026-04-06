# bkmr-memory Skill

Teaches AI agents (Claude Code, Copilot CLI, pi-mono) how to use [bkmr](https://github.com/sysid/bkmr) as persistent long-term memory across sessions.

## What It Does

- **READ**: Query bkmr for relevant memories at task start, before decisions, and when encountering bugs
- **WRITE**: Store non-trivial learnings with proper taxonomy and deduplication
- **UPDATE**: Enrich or correct existing memories with `bkmr update --url`
- **DEDUP**: Check for existing memories before creating new ones

Memories use the `_mem_` system tag and one of five classification tags: `fact`, `procedure`, `preference`, `episode`, `gotcha`. Optional `project:foo` tags scope memories to a project.

## Location

```
bkmr/skill/bkmr-memory/     # Source (lives in the bkmr repo)
~/.claude/skills/bkmr-memory # Symlink (makes it available to Claude Code)
```

## Files

```
bkmr-memory/
├── SKILL.md           # The skill itself (loaded into agent context)
├── README.md          # This file
└── evals/
    └── evals.json     # Test case definitions with assertions
```

Eval workspace (created at runtime, not checked in):
```
~/.claude/skills/bkmr-memory-workspace/
└── iteration-N/
    ├── benchmark.json
    ├── session-start-query/
    │   ├── eval_metadata.json
    │   ├── with_skill/
    │   │   ├── outputs/transcript.md
    │   │   ├── grading.json
    │   │   └── timing.json
    │   └── without_skill/
    │       └── (same structure)
    ├── post-task-memory-write/
    │   └── (same structure)
    └── dedup-scenario/
        └── (same structure)
```

## Running Evals

### Prerequisites

- `bkmr` installed and on PATH
- A bkmr database (test or scratch -- evals write to it)
- Claude Code CLI (`claude`) for spawning subagents

### Quick Regression Test

Run from Claude Code. The eval spawns 6 subagents (3 test cases x 2 configurations):

```
You: Run the bkmr-memory skill evals. Use the test cases in 
     ~/.claude/skills/bkmr-memory/evals/evals.json. 
     Create iteration-N in the workspace directory.
```

Or more explicitly:

```
You: For each test case in ~/.claude/skills/bkmr-memory/evals/evals.json, 
     spawn two subagents:
     1. WITH skill: read SKILL.md first, then execute the prompt
     2. WITHOUT skill: execute the prompt with only `bkmr --help`
     Save transcripts to ~/.claude/skills/bkmr-memory-workspace/iteration-N/
     Grade against the assertions in eval_metadata.json.
```

### Test Cases

| # | Name | Tests | Key Assertions |
|---|------|-------|----------------|
| 0 | session-start-query | READ workflow | Uses `-t _mem_`, checks gotchas/preferences, does NOT write |
| 1 | post-task-memory-write | WRITE workflow | Dedup first, `-t mem --no-web`, classification tag, content in url field |
| 2 | dedup-scenario | Dedup logic | Searches before writing, uses `_mem_` filter, updates or skips if match exists |

### Grading

Each run is graded against assertions in `eval_metadata.json`. The grading output goes in `grading.json` using this schema:

```json
{
  "expectations": [
    {
      "text": "assertion description",
      "passed": true,
      "evidence": "what was observed in the transcript"
    }
  ]
}
```

### Interpreting Results

From iteration-1 (2026-04-05):

| Metric | With Skill | Without Skill |
|--------|-----------|---------------|
| Pass rate | 100% (15/15) | 68% (10/15) |
| Avg duration | 74s | 170s |
| Avg tool calls | 8.3 | 14.3 |

Key failure modes without the skill:
- Content placed in `-d` (description) instead of url -- breaks embeddings and display
- No classification tags from the taxonomy
- No dedup check before writing
- No `_mem_` filter on read queries (searches entire DB)
- No category-specific queries (gotchas, preferences)

### Cleanup After Evals

Evals create real `_mem_` bookmarks in the database. Clean up with:

```bash
# List test memories
bkmr search -t _mem_ --json --np

# Delete by ID (interactive confirmation required)
bkmr delete 17,18,19
```

## Developing the Skill

### Editing SKILL.md

The skill body is loaded whenever Claude triggers it. Key constraints:
- Keep under 500 lines
- Every code example must be a complete, copy-pasteable command
- The "Mandatory Tag Rules" section at the top is the most important -- agents read top-down
- Memory content limit: 500 tokens max (url field)

### Key Concepts

- **url field IS the memory** -- first positional arg to `bkmr add`, used for embeddings and display
- **`-t mem`** on write automatically adds `_mem_` system tag
- **`-t _mem_`** on read filters to memory bookmarks only
- **`bkmr update --url "new content" <id>`** updates memory content without opening an editor
- **`bkmr update --title "new title" <id>`** fixes titles for better searchability
- **`project:foo`** tag structure scopes memories to a project

### Iteration Loop

1. Edit `SKILL.md`
2. Run all 3 eval test cases (with and without skill)
3. Grade against assertions
4. Compare to previous iteration's benchmark
5. Repeat until pass rate holds or improves

### Adding Test Cases

Add to `evals/evals.json`:

```json
{
  "id": 3,
  "name": "descriptive-name",
  "prompt": "Realistic user prompt",
  "expected_output": "What the agent should do",
  "files": [],
  "assertions": [
    {"text": "Observable behavior to check", "type": "required"}
  ]
}
```

Good test cases to add:
- **Memory update**: User corrects a previously stored fact (should use `bkmr update --url`)
- **Large content**: Agent receives a long code dump -- should summarize, not store verbatim
- **Irrelevant noise**: Agent finishes trivial work -- should NOT store anything
- **Multi-category query**: Task that requires checking facts AND gotchas AND procedures
- **Project scoping**: Memory stored with `project:foo` tag, queried with project filter

### Description Optimization

The `description` field in SKILL.md frontmatter controls when Claude triggers the skill. To optimize:

```
You: Use the skill-creator to optimize the description for bkmr-memory.
     Generate 20 trigger eval queries and run the optimization loop.
```

### Architecture Decisions

**Why url field for content, not description?**
The `_mem_` system tag's default action (`MemoryAction`) prints `bookmark.url` to stdout. Embeddings are computed from url + title + tags. Description is not embedded and not displayed.

**Why exactly one classification tag?**
Enables category-filtered queries (`-n gotcha`, `-n preference`). Multiple classifications would make a memory ambiguous -- is it a gotcha or a fact? Pick the primary one.

**Why `bkmr update --url` instead of `bkmr edit`?**
`bkmr update --url` is non-interactive and scriptable. `bkmr edit` opens an editor which agents cannot drive. For agents, `update --url` is the correct way to modify memory content.

**Why dedup before write?**
bkmr has no built-in uniqueness constraint on content. Without explicit dedup, agents accumulate near-identical memories that dilute search results.

**Why `--no-web` on every add?**
Without it, bkmr tries to fetch URL metadata (title, favicon) from the content string as if it were a URL. Memory content is plain text, not a URL -- the fetch fails or returns garbage.

**Why `project:foo` tag structure?**
Enables project-scoped queries (`-t project:myapp`) without polluting the flat tag namespace. The colon convention makes project tags visually distinct from classification and topic tags.
