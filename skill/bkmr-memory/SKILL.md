---
name: bkmr-memory
description: >
  Use bkmr as persistent long-term memory across agent sessions. This skill teaches how to store,
  query, deduplicate, and manage memories using bkmr's bookmark system with the _mem_ system tag.
  Use this skill whenever starting a new task (to recall relevant context), before architectural
  decisions (to check for precedence), and when encountering bugs or surprises (to check for gotchas).
  Also use when the user mentions remembering something, recalling past sessions, storing knowledge,
  agent memory, or cross-session persistence. Even if the user doesn't explicitly say "memory",
  trigger this skill when context from past work would clearly help the current task.
---

# bkmr-memory — Persistent Agent Memory

bkmr is a CLI bookmark manager with SQLite FTS5 and local semantic search (fastembed/ONNX).
Bookmarks tagged with the `_mem_` system tag serve as persistent agent memory that survives
across sessions. This skill defines how to read and write that memory effectively.

## Mandatory Tag Rules

Every interaction with bkmr memory requires specific tags. These are non-negotiable — without
them, memories are unfindable or uncategorized noise.

**READING — every query must include `-t _mem_`:**
```bash
bkmr hsearch "query" -t _mem_ --json --np       # hybrid search
bkmr search "query" -t _mem_ --json --np         # FTS search
bkmr search -t _mem_ -n gotcha --json --np       # tag-filtered search
```
The `-t _mem_` flag scopes results to agent memories only. Without it, you search the entire
bookmark database (personal bookmarks, snippets, shells, etc.) — guaranteed noise.

**WRITING — every memory must use `-t mem` AND exactly one classification tag:**
```bash
bkmr add "content" "CLASSIFICATION,topic1,topic2" --title "Title" -t mem --no-web
#                    ^^^^^^^^^^^^^^
#                    REQUIRED: exactly one of: fact, procedure, preference, episode, gotcha
```

The five classification tags:

| Tag | Use for | Example |
|-----|---------|---------|
| `fact` | Infrastructure truths, config, architecture | "Prod DB is PostgreSQL 15 on port 5433" |
| `procedure` | How-to steps, deployment, workarounds | "Deploy: make deploy-staging, then verify health" |
| `preference` | User conventions, tool choices, style | "User prefers pytest, uses fixtures not setUp" |
| `episode` | Session summaries, debugging narratives | "2026-04-04: JWT expiry was 5s not 5m" |
| `gotcha` | Non-obvious pitfalls, things that broke | "CI silently succeeds when registry is down" |

A memory without a classification tag cannot be filtered by category — it becomes a second-class
citizen in search results and breaks the taxonomy that makes memory useful.

## Optional Tag Rules

1. You may use topic tags for additional classification (e.g., `database`, `postgres`, `deployment`, `auth`).
2. To limit scope to a project, you may use a project tag. MANDATORY STRUCTURE for project tag: `project:foo`.  

---

## How It Works

A memory bookmark has three meaningful fields:

| Field | Purpose | Limit           |
|-------|---------|-----------------|
| `url` | **The memory content** (what you're storing) | max. 500 tokens |
| `title` | A concise, searchable title | Short phrase    |
| `tags` | Classification + topic tags | Comma-separated |

**The `url` field IS the memory.** This is the first positional argument to `bkmr add`. Do NOT
put memory content in the `-d` (description) field — that field is not used for embeddings or
the memory display action. If you put content in description instead of url, the memory becomes
unsearchable and displays as empty. Keep memories concise.

Embeddings for semantic search are computed from **content (url) + title + visible tags** (system
tags like `_mem_` are excluded from embeddings). This means your title and tags directly affect
how well the memory is found — make them descriptive.

---

## 1. Querying Memories (READ)

### When to Query

- **At every task start** — before doing any work, check for relevant memories
- **Before architectural decisions** — check for gotchas, preferences, procedures
- **When something feels familiar** — "didn't we solve this before?"
- **When encountering a bug or surprise** — check for past episodes or gotchas

### How to Query

Use `hsearch` (hybrid FTS + semantic) as the default — it handles imprecise queries well:

```bash
# Hybrid search — best for natural language queries
bkmr hsearch "database connection pooling" -t _mem_ --json --np

# With a classification filter — narrows results to a category
bkmr hsearch "deployment steps" -t _mem_ -n procedure --json --np

# Limit results when you only need top matches
bkmr hsearch "auth middleware" -t _mem_ --json --np -l 5
```

Use `search` (FTS-only) when you need exact keyword matching or structured queries:

```bash
# FTS5 exact phrase search
bkmr search '"JWT expiry"' -t _mem_ --json --np

# FTS5 boolean: find memories about auth BUT NOT about OAuth
bkmr search 'auth NOT oauth' -t _mem_ --json --np

# Tag-only search: all gotchas
bkmr search -t _mem_ -n gotcha --json --np

# Combined: FTS query filtered to preferences
bkmr search "testing framework" -t _mem_ -n preference --json --np -l 10
```

### Reading the Results

**hsearch --json** returns:
```json
[
  {
    "id": 42,
    "url": "The auth service uses JWT with 24h expiry. Refresh tokens are stored in Redis, not the DB.",
    "title": "Auth token architecture",
    "tags": ",_mem_,fact,auth,",
    "rrf_score": 0.032
  }
]
```

**search --json** returns:
```json
[
  {
    "id": 42,
    "url": "The auth service uses JWT with 24h expiry.",
    "title": "Auth token architecture",
    "tags": ["_mem_", "fact", "auth"],
    "access_count": 5,
    "created_at": "2026-01-15T10:30:00+00:00",
    "updated_at": "2026-03-20T14:00:00+00:00"
  }
]
```

The `url` field contains the actual memory content. The `rrf_score` (hsearch) indicates relevance
— higher is better. Use `id` to reference a specific memory for updates.

To view a single memory in detail:
```bash
bkmr show 42 --json
```

### Query Patterns by Situation

| Situation | Query |
|-----------|-------|
| Starting work on a project | `bkmr hsearch "project-name overview" -t _mem_ --json --np -l 10` |
| Before changing architecture | `bkmr hsearch "architecture decisions" -t _mem_ -n gotcha --json --np` |
| Checking user preferences | `bkmr search -t _mem_ -n preference --json --np` |
| Recalling a past debugging session | `bkmr hsearch "debugged auth issue" -t _mem_ -n episode --json --np` |
| Looking up a deployment procedure | `bkmr hsearch "deploy staging" -t _mem_ -n procedure --json --np` |

---

## 2. Storing Memories (WRITE)

### When to Store

Before completing any task, ask yourself:

> Did I learn anything non-trivial about this project, codebase, user preferences, or environment
> that would be valuable in a future session?

**Store it if** the insight is:
- Not derivable from reading source code or git history
- Not already in documentation (README, CLAUDE.md, comments)
- Likely to save time or prevent mistakes in a future session
- A user preference, convention, or decision rationale

**Do NOT store:**
- Code snippets (reference the file path instead)
- Anything already in CLAUDE.md or project docs
- Git history facts (`git log` is authoritative)
- Obvious things ("this project uses Rust" — the Cargo.toml says that)
- Temporary state or in-progress work details
- Full file contents or large artifacts

### Memory Taxonomy

Every memory gets exactly **one** classification tag:

| Tag | Use for | Content style |
|-----|---------|---------------|
| `fact` | Truths about infrastructure, config, architecture | Short declarative statement |
| `procedure` | How-to sequences, deployment steps, workarounds | Numbered steps or command sequence |
| `preference` | User conventions, tool choices, style decisions | "User prefers X over Y because Z" |
| `episode` | Session summaries, debugging narratives, decisions made | "On DATE, we did X because Y, learned Z" |
| `gotcha` | Non-obvious pitfalls, footguns, things that broke before | "X looks like it should work but fails because Y" |

### How to Store

```bash
bkmr add "CONTENT" "CLASSIFICATION,TOPIC_TAGS" --title "TITLE" -t mem --no-web
```

**Concrete examples:**

```bash
# Fact
bkmr add \
  "Production PostgreSQL 15 runs on db-prod-1.internal:5433. Read replicas on ports 5434-5436. Connection pool max is 50." \
  "fact,infrastructure,database" \
  --title "Production database connection details" \
  -t mem --no-web

# Procedure
bkmr add \
  "Deploy to staging: 1) Run 'make deploy-staging' 2) Wait for health check at /api/health 3) Verify logs in Grafana dashboard 'staging-deploys' 4) Run smoke tests with 'make test-smoke'" \
  "procedure,deployment,staging" \
  --title "Staging deployment procedure" \
  -t mem --no-web

# Preference
bkmr add \
  "User prefers pytest over unittest. Test files go in tests/ not __tests__/. Always use fixtures, never setUp/tearDown classes. Parametrize over duplicate test functions." \
  "preference,testing,python" \
  --title "Python testing conventions" \
  -t mem --no-web

# Episode
bkmr add \
  "2026-04-04: Debugged auth middleware timeout. Root cause: JWT expiry was set to 5 seconds (not 5 minutes) in config/auth.yml. The 's' suffix was interpreted as seconds, not minutes. Fix: changed '5s' to '5m'. See commit abc123." \
  "episode,auth,debugging" \
  --title "Auth middleware JWT expiry bug — 5s vs 5m" \
  -t mem --no-web

# Gotcha
bkmr add \
  "bkmr integration tests MUST run single-threaded (--test-threads=1) because they share a SQLite DB file. Parallel execution causes SQLITE_BUSY errors that look like test failures but are actually lock contention." \
  "project:bkmr,gotcha,testing" \
  --title "bkmr tests require single-threaded execution" \
  -t mem --no-web
```

### Content Guidelines

Since embeddings are built from content + title + tags, write memories that are **search-friendly**:

- **Title**: A specific, descriptive phrase — not "note" or "thing I learned"
- **Content**: Lead with the key fact. Add context after. Use concrete names, not pronouns.
- **Tags**: Include topic keywords beyond the classification (e.g., `fact,database,postgres,production`)
- **References over copies**: Instead of pasting code, write `"The retry logic in src/http/client.rs:45-60 uses exponential backoff with jitter. Max 3 retries, base delay 100ms."`

### Deduplication (Critical)

Before storing any memory, check if a similar one already exists:

```bash
# Search for similar memories
bkmr hsearch "your proposed memory content keywords" -t _mem_ --json --np -l 3
```

**Decision logic:**
- **No match** → create new memory
- **Outdated match** → update content with `bkmr update --url "..." <id>`
- **Same content exists** → skip, don't duplicate
- **Related but adds info** → update to merge both facts

### Update Examples

**Scenario 1: Outdated fact** — port changed from 5432 to 5433

```bash
# Step 1: Search finds existing memory
bkmr hsearch "production database" -t _mem_ -n fact --json --np -l 3
# Returns: id=42, url="Prod DB on port 5432", title="Production database config"

# Step 2: Update the content (port changed)
bkmr update --url "Production PostgreSQL 15 on port 5433. Pool max 50." 42
```

**Scenario 2: Enriching a memory** — adding new info to existing memory

```bash
# Step 1: Search finds a thin memory
bkmr hsearch "auth service" -t _mem_ --json --np -l 3
# Returns: id=87, url="Auth uses JWT with 24h expiry", title="Auth token config"

# Step 2: Enrich with more detail
bkmr update --url "Auth uses JWT with 24h expiry. Refresh tokens in Redis (not DB). Token rotation enabled. JWKS endpoint at /api/.well-known/jwks.json." 87
```

**Scenario 3: Fixing a bad title** — improving searchability

```bash
bkmr update --title "Auth service: JWT expiry, refresh tokens, JWKS endpoint" 87
```

**Scenario 4: Reclassifying a memory** — was stored as fact, should be gotcha

```bash
# Replace all tags (must include _mem_ and new classification)
bkmr update 87 -t "_mem_,gotcha,auth,jwt" -f
```

**Scenario 5: Adding topic tags** — make memory more discoverable

```bash
# Additive: keep existing tags, add new ones
bkmr update 87 -t redis,security
```

---

## 3. Decision Tree: Should I Store This?

```
Did I learn something new?
├── No → Don't store
└── Yes
    ├── Is it in the source code, docs, or git history?
    │   ├── Yes → Don't store (it's already findable)
    │   └── No
    │       ├── Is it trivial or obvious?
    │       │   ├── Yes → Don't store
    │       │   └── No
    │       │       ├── Would a future session benefit from knowing this?
    │       │       │   ├── No → Don't store
    │       │       │   └── Yes
    │       │       │       ├── Does a similar memory already exist?
    │       │       │       │   ├── Yes → Update existing (bkmr update --url "..." <id>)
    │       │       │       │   └── No → Create new memory
    │       │       │       └── Done
    │       │       └── Done
    │       └── Done
    └── Done
```

---

## 4. Anti-Patterns

| Anti-pattern | Why it's bad | Do this instead |
|-------------|-------------|----------------|
| Putting content in `-d` instead of url | Breaks embeddings AND memory display | First positional arg = content |
| Missing `-t mem` flag | Memory has no `_mem_` tag, invisible to queries | Always use `-t mem` when adding |
| Missing classification tag | Unfilterable, breaks taxonomy | Always include exactly one of: fact/procedure/preference/episode/gotcha |
| Missing `-t _mem_` on queries | Searches entire DB, returns noise | Every read query needs `-t _mem_` |
| Storing full file contents | Exceeds token limit, stale immediately | Store summary + file path reference |
| Storing git history | `git log` is authoritative and current | Only store *interpretations* ("we chose X because Y") |
| Storing TODO items | Ephemeral, belongs in issue tracker | Store the *decision* or *reason*, not the task |
| Vague titles like "note" | Unsearchable, useless embeddings | Use specific descriptive titles |
| Duplicate memories | Noise drowns signal | Always dedup before writing |
| Storing things from CLAUDE.md | Already in context every session | Only store what's NOT in docs |
| Over-tagging | Dilutes search relevance | 1 classification + 2-4 topic tags max |
| Storing every session | Most sessions are routine | Only store sessions with non-obvious learnings |

---

## 5. Session Workflow Summary

### At Session Start
```bash
# 1. Query for project-relevant memories
bkmr hsearch "<project-name> <current-task-keywords>" -t _mem_ --json --np -l 10

# 2. Check for gotchas related to what you're about to do
bkmr hsearch "<area-of-work>" -t _mem_ -n gotcha --json --np

# 3. Check user preferences if making style/tool decisions
bkmr search -t _mem_ -n preference --json --np -l 10
```

### During Work
```bash
# Before architectural decisions, check for prior decisions/gotchas
bkmr hsearch "<decision topic>" -t _mem_ --json --np
```

### At Session End
```bash
# 1. Reflect: did I learn anything non-trivial?
# 2. If yes, dedup check:
bkmr hsearch "<summary of what I learned>" -t _mem_ --json --np -l 3
# 3. If no duplicate, store it:
bkmr add "concise memory content" "classification,topic1,topic2" \
  --title "Descriptive title" -t mem --no-web
```
