# bkmr-memory Skill

Teaches AI agents (Claude Code, Copilot CLI, pi-mono) how to use [bkmr](https://github.com/sysid/bkmr) as persistent long-term memory across sessions.

## What It Does

- **READ**: Query bkmr for relevant memories at task start, before decisions, and when encountering bugs
- **WRITE**: Store non-trivial learnings with proper taxonomy and deduplication
- **UPDATE**: Enrich or correct existing memories with `bkmr update --url`
- **KEEP CURRENT**: Check constraints before relying on them; correct or delete what evidence contradicts, without asking the user, and stamp what it confirms. History keeps the old state
- **DEDUP**: Check for existing memories before creating new ones
- **FORMAT**: Store content as light markdown that also reads well for humans (lead line, prose
  wrapped at 100 columns, fenced commands, `1.` lists); no headings or tables, which hurt retrieval

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
├── scripts/
│   ├── due-report.py           # Lists memories due for review; never changes the store
│   ├── run-audit.py            # One headless audit over the most overdue memories
│   └── test_*.py               # Tests for both
└── evals/
    ├── evals.json              # Agent-behaviour test cases with assertions
    ├── lifecycle_eval.py       # Fixture and grader for the lifecycle evals
    ├── markdown_retrieval.py   # Deterministic retrieval eval (markdown vs plain)
    └── markdown_corpus.json    # Corpus + queries for markdown_retrieval.py
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

## Reviewing Due Memories

Agents keep memories current while they work, as SKILL.md's "Keeping Memory Current" asks: they
check a memory before relying on it, correct what evidence contradicts, and stamp what they
confirm with `bkmr verify`. That only reaches memories somebody uses. A memory nobody touches, or
one about something that changed outside any session (a tool upgrade, infrastructure), stays
wrong. The two scripts cover those:

```
due-report.py ──> which memories are due?              read only, run any time
      │
      v
run-audit.py  ──> headless agent reviews the 20 most    writes to the store (undoable),
                  overdue, under SKILL.md's rules       then a digest for you to read
```

Both are stdlib-only `uv` scripts. Inside this repo, direnv puts `bkmr/target/debug/bkmr` first on
PATH, which may be an older build of whatever branch is checked out. Pass the release binary with
`--bkmr ~/bin/bkmr`; the scripts refuse a bkmr older than 8.2.0 because its output has no
`verified_at`.

### When a memory is due

Age counts from `verified_at` (the last `bkmr verify`), or from `created_at` if the memory was
never stamped or was changed since; a change to content or title clears the stamp. A memory is due
once its age reaches its window:

| Class | Window |
|-------|--------|
| `fact` | 90 days |
| `procedure`, `gotcha`, `episode` | 180 days |
| `preference` | 365 days |
| No class | 90 days, flagged |
| Pins a version (`1.24.4`, `v2`, `-rc.13`) or has a harness tag (`sandbox`, `claude-code`, `codex`, `mcp`, ...) | at most 90 days |

A memory with two classes gets the shorter window and a flag. Due means "audit", never "delete".

### due-report.py: see what is due

Use it to look at the backlog, before an audit, or to check that stamps took effect. It only runs
`bkmr search`, which changes nothing.

```bash
scripts/due-report.py --bkmr ~/bin/bkmr --db ~/path/to/bm.db
```

```
32 of 156 memories due
    id  over  win from     anchor     chk  title / flags
  3505   124   90 created  2026-04-07 yes  ort-sys API version must not exceed Homebrew onnxruntime
                                          pinned: version
```

| Column | Meaning |
|--------|---------|
| `over` | Days past the window; the list is sorted by it, most overdue first |
| `win` | Review window in days, after class, pins and flags |
| `from`, `anchor` | Whether age counts from `verified` or `created`, and that date |
| `chk` | `yes` if content or description cites a path, commit or URL, so an agent can recheck it. `-` means the audit will most likely keep it as a hint |

Options: `--json` prints the same list as JSON; `--input FILE` reads a saved
`bkmr search -t _mem_ --json` instead of calling bkmr; `--now YYYY-MM-DD` evaluates as of another
date.

### run-audit.py: review the due memories

One run hands the 20 most overdue memories to a headless agent (`claude -p`). For each one it reads
the memory, looks for direct evidence in the code, config and lock files it cites, and then:

| Finding | Action |
|---------|--------|
| Evidence confirms all of it | Stamped with `bkmr verify` |
| Evidence contradicts it | Corrected in place, evidence in the description |
| Nothing true is left, or junk | Reason saved, then deleted |
| Partly confirmed, not reproduced, or not cheaply checkable | Kept unchanged |
| A preference, or a wrong instruction file such as `CLAUDE.md` | Reported under "Needs Tom", unchanged |

It stops changing memories after more than `min(5, 10%)` of those it reviews (2 for 20), so a
bad run cannot rewrite the store. Stamps do not count.

**What the agent may do.** It runs with `--permission-mode dontAsk`, without user or project
settings and without MCP servers, so only these tools work: Read, Grep, Glob, `git log`, `git
show`, and `bkmr search`, `hsearch`, `show`, `history`, `update`, `verify` and `delete --yes`. It
cannot edit files, run `bkmr open` (which executes shell bookmarks), purge with `delete --force`,
or reach the network. User settings are left out on purpose: a user-level `Bash` allow rule would
otherwise make the list meaningless.

**How to use it:**

1. Preview the prompt and the exact `claude` command. Nothing runs:

   ```bash
   scripts/run-audit.py --bkmr ~/bin/bkmr --db ~/path/to/bm.db --dry-run
   ```
2. Run the first audit on a copy, so you can judge the agent's decisions before it touches the
   real store:

   ```bash
   sqlite3 ~/path/to/bm.db ".backup '/tmp/bm-audit.db'"
   scripts/run-audit.py --bkmr ~/bin/bkmr --db /tmp/bm-audit.db --out-dir /tmp/bkmr-audit
   ```
3. Read the digest the script prints the path of. Its header comes from the store, not from the
   agent: changed, deleted and stamped IDs, whether the threshold was exceeded, and the cost.
   Then the agent's line per memory with its evidence, and "Needs Tom".
4. Undo anything you disagree with; the digest lists one command per change:

   ```bash
   bkmr restore <history_id>
   ```
5. Once the decisions look right, run it on the real store. The digest goes to
   `~/.local/state/bkmr-audit/<date>.md`. Repeat monthly, or until the backlog is gone: the first
   runs work through memories that were never stamped.

| Exit code | Meaning |
|-----------|---------|
| 0 | Done, or nothing due (then no agent runs) |
| 1 | The agent failed, e.g. auth or credit; the digest holds the error |
| 2 | More changes than the threshold: read the digest and restore what is wrong |

Options: `--limit N` memories per run (default 20), `--model` (default `claude-opus-5-5`),
`--budget-usd` cap per run (default 5), `--out-dir`, `--claude` binary.

**Auth.** `claude -p` uses `ANTHROPIC_API_KEY` if it is set, otherwise your claude.ai login.
Decide which one the audit should bill before running it.

**Not yet done:** no live run has confirmed that leaving out user settings keeps a user-level
`Bash` rule out; check the first digest for commands outside the list above. There is no schedule:
a cloud routine cannot reach the local database, so a monthly run needs launchd or cron on this
machine.

Run the tests with `make -C skill test`; `make -C skill lint` checks every Python script in `skill/`.

## Running Evals

### Prerequisites

- `bkmr` installed and on PATH
- A bkmr database (test or scratch -- evals write to it)
- Claude Code CLI (`claude`) for spawning subagents

### Quick Regression Test

Run from Claude Code. The eval spawns 8 subagents (4 test cases x 2 configurations):

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
| 3 | shell-quoting | WRITE safety | No double-quoted content; backticks/`$`/apostrophes stored verbatim; no `--stdin` + positional tags |

### Lifecycle Evals: Keeping Memory Current

Seven more test cases in `evals.json` check how an agent handles memories that may be wrong. They run
against a throwaway repo and store built by `evals/lifecycle_eval.py`, which grades the end state
without an LLM and never touches a real DB. Each run needs its own directory:

```bash
evals/lifecycle_eval.py setup /tmp/lc/r1
evals/lifecycle_eval.py prompt /tmp/lc/r1 stale-deploy-flag   # give this text to a subagent
evals/lifecycle_eval.py grade /tmp/lc/r1 stale-deploy-flag
```

Keep eval names out of the run paths and out of the agent's view: a path like `stale-deploy-flag`
tells the agent the answer. Put `SKILL.md` in the parent of the run directories.

Results (2026-11-07, one run per case, Opus 5.5, bkmr 8.0.0, so history kept the old text for both):

| Skill | Assertions passed | Both stale memories fixed | Asked the user about a stale memory | Mean tool calls | Mean tokens |
|---|---|---|---|---|---|
| Before lifecycle rules | 28/32 | 4/7 runs | 3/7 runs | 8.0 | 60.6K |
| With lifecycle rules | 32/32 | 7/7 runs | 0/7 runs | 9.7 | 65.8K |

All four failed assertions are a missing reason in `-d`. No run on either skill changed a memory that
still held. These are small repos and one run each, in a store where two of eight memories were stale:
read the table as direction, not as a rate.

Five of the 14 runs tried to run `deploy.sh` to test a memory, and the auto mode classifier blocked
each one. A sentence against this was added afterwards: re-running the three affected cases gave one
attempt in three and the same grades. That is too few runs to show the sentence works, so do not rely on
it as a safeguard. The runs also shared port 8123, which fits the few `make test` failures they saw, but that is not proven.

`bkmr verify` was checked afterwards on four of these cases (2026-11-07, one run each): 24/25
assertions passed. Every run stamped its correction, none stamped the memory no repo can confirm, and the
memory the user reported stayed unstamped. The failed assertion is a run that reworded a memory that
still held, to say that part of it rests only on a code comment. One run stamped a memory it had only
partly confirmed, so the sentence on partial stamps was added afterwards. It has not been tested.

### Retrieval Eval: Markdown vs. Plain Content

Deterministic, no LLM involved. `evals/markdown_retrieval.py` loads the same 24 facts
(`evals/markdown_corpus.json`) plus 24 distractors into throwaway DBs in four renderings and
ranks the target for 48 queries (half keyword, half paraphrase). Never touches the real DB.

```bash
./evals/markdown_retrieval.py                 # ~4 min, prints tables
./evals/markdown_retrieval.py --keep /tmp/x --json /tmp/x/results.json
```

Results (2026-11-03, NomicEmbedTextV15, bkmr 7.8.0):

| Variant | hsearch MRR | R@1 (paraphrase) | Embedding tokens (mean) |
|---|---|---|---|
| plain | 0.924 | 0.75 | 83 |
| md_light (inline code, bullets) | 0.914 | 0.71 | 85 |
| md_heavy (headings, tables, links) | 0.912 | 0.75 | 150 |
| consolidated (4 facts per `##` doc) | 0.864 | 0.58 | — |

Truncation probe: a key fact after a 60-row table (1540 tokens > 512 window) ranks 22 instead of 1.

Takeaways: markdown syntax itself is neutral (plain/light/heavy differ by ~1 query out of 48,
within noise; FTS5 `unicode61` drops the punctuation). What hurts is what markdown invites:
multi-fact documents and content long enough to be truncated. Keyword queries scored 1.0 for
hsearch in every variant; FTS hit 0% of paraphrase queries because `hsearch` ANDs all terms,
so paraphrase recall is carried entirely by the semantic arm.

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

# Delete by ID (asks for confirmation; --yes skips it). History keeps the text of a memory:
# use --force as well if an eval stored anything sensitive
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
- **`bkmr update --url 'new content' <id>`** updates memory content without opening an editor
- **`bkmr update --title 'new title' <id>`** fixes titles for better searchability
- **Single quotes, never double** around content -- backticks/`$` in double quotes are executed/expanded by the shell; use `"$(cat <<'EOF' ... EOF)"` when content contains `'`
- **`project:foo`** tag structure scopes memories to a project

### Iteration Loop

1. Edit `SKILL.md`
2. Run all 4 agent eval test cases (with and without skill)
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
