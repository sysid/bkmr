---
name: bkmr-memory
description: >
  Primary memory system — supersedes built-in file-based memory (MEMORY.md / memory/ directory).
  All memory reads and writes go through bkmr, not the filesystem. Invoke for ANY of:
  (1) User asks about memories, past sessions, what you remember — "what do you know about X".
  (2) User asks to remember or save something for future sessions.
  (3) Starting a new task — check for relevant context, gotchas, or preferences.
  (4) Encountered a bug or surprise — check if seen before.
  (5) Making an architectural or tooling decision — check for prior decisions.
  (6) End of task — persist non-trivial learnings.
  When in doubt, invoke — a quick bkmr query costs almost nothing.
  DO NOT use file-based memory — use bkmr instead.
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
bkmr hsearch 'query' -t _mem_ --json --np       # default: hybrid (FTS + semantic)
```
The `-t _mem_` flag scopes results to agent memories only. Without it, you search the entire
bookmark database (personal bookmarks, snippets, shells, etc.) — guaranteed noise.

Always use `hsearch` as your search command. It combines FTS and semantic ranking via
Reciprocal Rank Fusion, so it handles both keyword and conceptual queries well.
Only fall back to `search` (FTS-only) when you specifically need exact phrase matching
(`'"JWT expiry"'`), boolean logic (`'auth NOT oauth'`), or tag-only browsing with no
query text (`-t _mem_ -n gotcha`).

**WRITING — every memory must use `-t mem` AND exactly one classification tag:**
```bash
bkmr add "$(cat <<'EOF'
<lead sentence: the key fact>

<context, wrapped at 100 columns; commands in fenced blocks>
EOF
)" 'CLASSIFICATION,topic1,topic2' --title 'Specific title' -t mem --no-web
#   ^^^^^^^^^^^^^^ REQUIRED: exactly one of: fact, procedure, preference, episode, gotcha
```

The five classification tags:

| Tag | Use for | Content shape |
|-----|---------|---------------|
| `fact` | Infrastructure truths, config, architecture | Lead sentence, then `-` bullets for details |
| `procedure` | How-to steps, deployment, workarounds | Lead sentence, then a `1.` numbered list or fenced commands |
| `preference` | User conventions, tool choices, style | "User prefers X over Y because Z", then bullets |
| `episode` | Debugging narratives, surprising discoveries | "DATE: X failed because Y." then a paragraph with the fix |
| `gotcha` | Non-obvious pitfalls, things that broke | "X looks like it works but fails because Y." then the fix |

A memory without a classification tag cannot be filtered by category — it becomes a second-class
citizen in search results and breaks the taxonomy that makes memory useful.

## Optional Tag Rules

1. You may use topic tags for additional classification (e.g., `database`, `postgres`, `deployment`, `auth`).
2. To limit scope to a project, you may use a project tag. MANDATORY STRUCTURE for project tag: `project:foo`.
3. **No redundant tags.** If you use a project tag `project:foo`, do NOT also add a bare `foo` tag —
   `project:foo` already carries the project identity. Same rule for any topic expressed as both a
   namespaced and bare tag (e.g., don't pair `lang:python` with `python`). Pick the more specific
   form and drop the other. Redundant tags dilute embedding signal and waste the 2–4 topic-tag budget.
   Topic tags should name the *subject matter* (e.g., `fzf`, `skim`, `bash`), not repeat the project.

---

## How It Works

A memory bookmark has three meaningful fields:

| Field | Purpose | Limit           |
|-------|---------|-----------------|
| `url` | **The memory content** (formatted markdown, see "Formatting the Content") | max. 500 tokens |
| `title` | A concise, searchable title | Short phrase    |
| `tags` | Classification + topic tags | Comma-separated |

**The `url` field IS the memory.** This is the first positional argument to `bkmr add`. Do NOT
put memory content in the `-d` (description) field — that field is not used for embeddings or
the memory display action. If you put content in description instead of url, the memory becomes
unsearchable and displays as empty. Keep memories concise.

Embeddings for semantic search are computed from **content (url) + title + visible tags** (system
tags like `_mem_` are excluded from embeddings). This means your title and tags directly affect
how well the memory is found — make them descriptive. Memories are always embedded by default
(`_mem_ = "always"` in `[embeddings.policy]`); `--no-embed` is overridden with a warning.

---

## 1. Querying Memories (READ)

### When to Query

- **At every task start** — before doing any work, check for relevant memories
- **Before architectural decisions** — check for gotchas, preferences, procedures
- **When something feels familiar** — "didn't we solve this before?"
- **When encountering a bug or surprise** — check for past episodes or gotchas

### How to Query

Use **one** `hsearch` call per lookup. Do not run both `hsearch` and `search` for the same
query — the results overlap heavily and waste context.

`hsearch` matches every term literally — `c++`, `node.js`, `foo-bar` need no quoting.
FTS5 syntax (phrases, `AND`/`OR`/`NOT`, `prefix*`) only works in `search`.

```bash
# Standard query — handles keywords and conceptual matches
bkmr hsearch 'database connection pooling' -t _mem_ --json --np

# With a classification filter — narrows results to a category
bkmr hsearch 'deployment steps' -t _mem_ -n procedure --json --np

# Limit results when you only need top matches
bkmr hsearch 'auth middleware' -t _mem_ --json --np -l 5
```

**FTS-only fallback** — use `search` instead of `hsearch` only for these three cases:

```bash
# 1. Exact phrase match (quotes inside the query)
bkmr search '"JWT expiry"' -t _mem_ --json --np

# 2. Boolean logic (AND/OR/NOT)
bkmr search 'auth NOT oauth' -t _mem_ --json --np

# 3. Tag-only browse with no query text
bkmr search -t _mem_ -n gotcha --json --np
```

### Reading the Results

`hsearch --json` returns:
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

The `url` field contains the actual memory content. The `rrf_score` indicates relevance —
higher is better. Use `id` to reference a specific memory for updates.

To view a single memory in detail: `bkmr show 42 --json`

Note: `search --json` returns a slightly different shape (tags as array, includes
`access_count`/`created_at`/`updated_at` instead of `rrf_score`), but the key fields
(`id`, `url`, `title`, `tags`) are the same.

### Query Patterns by Situation

| Situation | Query |
|-----------|-------|
| Starting work on a project | `bkmr hsearch 'project-name overview' -t _mem_ --json --np -l 5` |
| Before changing architecture | `bkmr hsearch 'architecture decisions' -t _mem_ -n gotcha --json --np` |
| Checking user preferences | `bkmr hsearch 'conventions style' -t _mem_ -n preference --json --np` |
| Recalling a past debugging session | `bkmr hsearch 'debugged auth issue' -t _mem_ -n episode --json --np` |
| Looking up a deployment procedure | `bkmr hsearch 'deploy staging' -t _mem_ -n procedure --json --np` |
| Browse all gotchas (no query) | `bkmr search -t _mem_ -n gotcha --json --np` |

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
- **What you changed** — file lists, renames, refactoring summaries, documentation updates.
  All of this is recoverable from `git log`, `git diff`, and reading the code.
  A memory that says "Updated files X, Y, Z" or "Renamed A to B" is pure noise.
- **Anything derivable by reading the codebase** — project structure, which classes exist,
  what frameworks are used, how modules are wired together. The code is the source of truth;
  re-reading it is cheap. Only store *interpretations* that aren't obvious from the code
  (e.g., "X looks wrong but is intentional because of Y constraint").

Every memory gets exactly **one** classification tag — see the table in "Mandatory Tag Rules".

### How to Store

````bash
bkmr add "$(cat <<'EOF'
<lead sentence: the key fact>

<context, wrapped at 100 columns>

```bash
<command someone would run>
```
EOF
)" 'CLASSIFICATION,TOPIC_TAGS' --title 'TITLE' -t mem --no-web
````

### Formatting the Content

Agents are the main readers: they get the content as one JSON string (`\n` for line breaks) and
it feeds search. Humans read the same text in a terminal or rendered as markdown. Format for
both, and when the two conflict, the agent wins.

1. **Lead line.** The first line is the key fact as a complete sentence. Skimming agents and
   humans read it first; it also anchors the embedding.
2. **Wrap prose at 100 columns.** Break at spaces. Never split inline code, a path, or a URL;
   let that line run long instead. Column counts need not be exact: about 15 words fill 100
   columns, and breaking earlier is always harmless.
3. **Blank line between paragraphs**, one idea each. Structure what you were told; do not pad
   it. A one-sentence fact stays one paragraph, and a missing *why* or *fix* stays missing:
   every added claim is unverified and a future session will trust it.
4. **Fenced code blocks** with a language tag (` ```bash `, ` ```yaml `) for anything someone
   would run or paste: commands, config, multi-line code. Never wrap lines inside a fence; they
   get copy-pasted. Single identifiers, flags, paths and values stay `inline code`.
5. **Lists:** steps as `1.`, parallel details as `-`, one item per line. A code fence that
   belongs to a step is indented 3 spaces (aligned with the step text), otherwise it ends the
   list:
   ````
   1. Convert the cert:

      ```bash
      keytool -importkeystore -srckeystore new.p12 -destkeystore kafka.jks
      ```
   2. Copy `kafka.jks` to every broker.
   ````
6. **No headings, tables or `[text](url)` links.** The retrieval eval
   (`evals/markdown_retrieval.py`) showed why: a memory that needs `##` sections is several facts
   merged into one blurred embedding (paraphrase R@1 dropped from 0.75 to 0.58), and a table can
   push the tail past the 512-token embedding window, where it is silently cut. Split instead.

This formatting is free for retrieval. Measured with bkmr's embedding tokenizer: line breaks cost
0 tokens (the tokenizer treats them as spaces, and so does FTS), a paragraph break about 1, a code
fence about 4. The quoted heredoc below stores line breaks, backticks and `$` exactly as typed.

### Shell Quoting (Critical)

Memory content routinely contains backticks, `$`, and `!`. Inside **double quotes** the shell
rewrites them *before bkmr sees the text* — and it fails silently:

```bash
bkmr add "Run `date +%Y` first; costs $5" ...   # stored: "Run 2026 first; costs "  ← corrupted, and `date` was EXECUTED
```

Rules — they apply to `add`, `update --url`, `--title`, and queries alike:

1. **Content: always a quoted heredoc via command substitution.** The quoted delimiter `'EOF'`
   disables all expansion and the result is not re-parsed, so line breaks, backticks, `$` and
   apostrophes all survive. One form for every memory means no per-memory judgement call:
   ```bash
   bkmr update --url "$(cat <<'EOF'
   Don't run `make clean` before deploy; it wipes $CACHE_DIR.
   EOF
   )" 42
   ```
   Heredoc lines start at column 0: any indentation is stored as part of the content.
2. **Titles, tags and queries: single quotes.** Nothing inside `'...'` is expanded. Phrase
   titles without apostrophes.
3. **Never** use double quotes around content, and never use `--stdin` with a positional content
   argument: `bkmr add --stdin 'fact,x'` parses `'fact,x'` as the URL slot, discards it, and
   silently drops every tag except `_mem_`.

**Concrete examples** (the heredoc body is exactly what gets stored):

````bash
# Fact
bkmr add "$(cat <<'EOF'
Production PostgreSQL 15 runs on `db-prod-1.internal:5433`, not the default port 5432.

- Read replicas: ports 5434-5436 on the same host
- Connection pool max: 50
EOF
)" 'fact,infrastructure,database' --title 'Production database connection details' -t mem --no-web

# Procedure
bkmr add "$(cat <<'EOF'
Staging deploys take four steps; the smoke tests are the only check that catches a broken
migration.

1. Run `make deploy-staging`.
2. Wait until `/api/health` returns 200.
3. Check the Grafana dashboard `staging-deploys` for errors.
4. Run `make test-smoke`.
EOF
)" 'procedure,deployment,staging' --title 'Staging deployment procedure' -t mem --no-web

# Preference
bkmr add "$(cat <<'EOF'
User prefers pytest over unittest.

- Test files go in `tests/`, not `__tests__/`.
- Use fixtures, never setUp/tearDown classes.
- Parametrize instead of duplicating test functions.
EOF
)" 'preference,testing,python' --title 'Python testing conventions' -t mem --no-web

# Episode
bkmr add "$(cat <<'EOF'
2026-04-04: the auth middleware timed out because the JWT expiry in `config/auth.yml` was 5
seconds, not 5 minutes.

The value `5s` reads like five minutes at a glance, but the `s` suffix means seconds. Fix:
changed it to `5m` (commit abc123).
EOF
)" 'episode,auth,debugging' --title 'Auth middleware JWT expiry bug: 5s vs 5m' -t mem --no-web

# Gotcha
bkmr add "$(cat <<'EOF'
bkmr integration tests must run single-threaded because they share one SQLite DB file.

Parallel runs fail with `SQLITE_BUSY`, which looks like a test failure but is lock contention:

```bash
cargo test -- --test-threads=1
```
EOF
)" 'project:bkmr,gotcha,testing' --title 'bkmr tests require single-threaded execution' -t mem --no-web
````

### Content Guidelines

Since embeddings are built from content + title + tags, write memories that are **search-friendly**:

- **Title**: A specific, descriptive phrase — not "note" or "thing I learned"
- **Content**: Lead with the key fact. Add context after. Use concrete names, not pronouns.
- **Tags**: Include topic keywords beyond the classification (e.g., `fact,database,postgres,production`)
- **References over copies**: Instead of pasting code, write `"The retry logic in src/http/client.rs:45-60 uses exponential backoff with jitter. Max 3 retries, base delay 100ms."`
- **Formatting is free, structure is not.** Line breaks, paragraphs, bullets and code fences cost
  retrieval nothing; headings and tables do (see "Formatting the Content"). A table that pushed a
  fact past the 512-token embedding window dropped it from rank 1 to 22.

### Deduplication (Critical)

Before storing any memory, check if a similar one already exists:

```bash
# Search for similar memories
bkmr hsearch 'your proposed memory content keywords' -t _mem_ --json --np -l 3
```

**Decision logic:**
- **No match** → create new memory
- **Outdated match** → update content with `bkmr update --url '...' <id>`
- **Same content exists** → skip, don't duplicate
- **Related but adds info** → update to merge both facts

### Update Examples

**Scenario 1: Outdated fact** — port changed from 5432 to 5433

```bash
# Step 1: Search finds existing memory
bkmr hsearch 'production database' -t _mem_ -n fact --json --np -l 3
# Returns: id=42, url="Prod DB on port 5432", title="Production database config"

# Step 2: Update the content (port changed); the new content replaces the old one entirely
bkmr update --url "$(cat <<'EOF'
Production PostgreSQL 15 runs on port 5433, not the default 5432.

- Connection pool max: 50
EOF
)" 42
```

**Scenario 2: Enriching a memory** — adding new info to existing memory

```bash
# Step 1: Search finds a thin memory
bkmr hsearch 'auth service' -t _mem_ --json --np -l 3
# Returns: id=87, url="Auth uses JWT with 24h expiry", title="Auth token config"

# Step 2: Enrich with more detail; keep the old facts, add the new ones
bkmr update --url "$(cat <<'EOF'
Auth uses JWT with a 24h expiry; refresh tokens live in Redis, not the database.

- Token rotation is enabled.
- JWKS endpoint: `/api/.well-known/jwks.json`
EOF
)" 87
```

**Scenario 3: Fixing a bad title** — improving searchability

```bash
bkmr update --title 'Auth service: JWT expiry, refresh tokens, JWKS endpoint' 87
```

**Scenario 4: Reclassifying a memory** — was stored as fact, should be gotcha

```bash
# Replace all tags (must include _mem_ and new classification)
bkmr update 87 -t '_mem_,gotcha,auth,jwt' -f
```

**Scenario 5: Adding topic tags** — make memory more discoverable

```bash
# Additive: keep existing tags, add new ones
bkmr update 87 -t redis,security
```

---

## 3. Checklist: Should I Store This?

Store only if **all four** hold; the first "no" ends the check:

1. I learned something new and non-trivial.
2. It is not in the code, docs, CLAUDE.md or git history.
3. I could not re-derive it from code + `git log` in under 5 minutes (re-deriving beats a stale
   memory).
4. A future session would act differently knowing it.

Then dedup: a similar memory exists → update it (`bkmr update --url … <id>`); otherwise add a new
one.

---

## 4. Anti-Patterns

| Anti-pattern | Why it's bad | Do this instead |
|-------------|-------------|----------------|
| Putting content in `-d` instead of url | Breaks embeddings AND memory display | First positional arg = content |
| Missing `-t mem` flag | Memory has no `_mem_` tag, invisible to queries | Always use `-t mem` when adding |
| Missing classification tag | Unfilterable, breaks taxonomy | Always include exactly one of: fact/procedure/preference/episode/gotcha |
| Missing `-t _mem_` on queries | Searches entire DB, returns noise | Every read query needs `-t _mem_` |
| Storing full file contents | Exceeds token limit, stale immediately | Store summary + file path reference |
| Double-quoting content (`bkmr add "..."`) | Backticks run as commands, `$VAR` expands — memory silently corrupted | Quoted heredoc `"$(cat <<'EOF' ... EOF)"` |
| `bkmr add --stdin 'tags'` | Tags land in the URL slot and are discarded | Use `"$(cat <<'EOF' ... EOF)"` as the content argument |
| Multi-section markdown memory (`##`, tables) | One blurred embedding for many facts; tail truncated past 512 tokens | One fact per memory; split the sections into separate memories |
| One-line wall of text, steps as `1) … 2) …` | Unreadable for humans; commands cannot be copied | Lead line, wrapped paragraphs, `1.` lists, fenced commands |
| Storing git history | `git log` is authoritative and current | Only store *interpretations* ("we chose X because Y") |
| Storing TODO items | Ephemeral, belongs in issue tracker | Store the *decision* or *reason*, not the task |
| Vague titles like "note" | Unsearchable, useless embeddings | Use specific descriptive titles |
| Duplicate memories | Noise drowns signal | Always dedup before writing |
| Storing things from CLAUDE.md | Already in context every session | Only store what's NOT in docs |
| Over-tagging | Dilutes search relevance | 1 classification + 2-4 topic tags max |
| Redundant project + bare tag (`project:bkmr` AND `bkmr`) | Namespaced tag already carries the identity; the bare tag is noise | Keep only `project:bkmr`; use remaining topic tags for subject matter (`fzf`, `bash`, …) |
| Storing every session | Most sessions are routine | Only store sessions with non-obvious learnings |
| Storing what you changed | Git log is authoritative; file lists, renames, refactorings are noise | Store the *insight* or *decision rationale*, not the change list |
| Storing codebase structure | Reading the code is cheap; structure changes with every PR | Only store non-obvious *interpretations* (e.g., "X exists because of Y constraint") |

---

## 5. Session Workflow (MANDATORY)

Memory only pays off if it is read before work and written after it; a session that skips either
step makes the whole store dead weight. So run the actual bkmr commands, do not just read this
section.

### At Session Start (BEFORE any work)
Query for relevant memories before writing code, making plans, or answering questions.
A single well-crafted hsearch is usually enough — don't run multiple overlapping queries.
```bash
# Primary query — combine project name + task keywords for best recall
bkmr hsearch '<project-name> <current-task-keywords>' -t _mem_ --json --np -l 5

# Only add a second query if the task involves a specific risk area
bkmr hsearch '<area-of-work>' -t _mem_ -n gotcha --json --np
```

### During Work (BEFORE decisions, AT strange bugs)
```bash
# Before architectural decisions, check for prior decisions/gotchas
bkmr hsearch '<decision topic>' -t _mem_ --json --np
```

### At Task End or Session End (BEFORE signing off)
Evaluate whether anything learned warrants storing, using the checklist in section 3. If something
qualifies, dedup and store it, formatted as in "Formatting the Content":
```bash
# 1. Reflect: did I learn anything non-trivial that isn't in code/docs/git?
# 2. If yes, dedup check:
bkmr hsearch '<summary of what I learned>' -t _mem_ --json --np -l 3
# 3. If no duplicate, store it:
bkmr add "$(cat <<'EOF'
<lead sentence>

<context, wrapped at 100 columns>
EOF
)" 'classification,topic1,topic2' --title 'Descriptive title' -t mem --no-web
# 4. If nothing worth storing, that's fine, but you must have evaluated.
```
