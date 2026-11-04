#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = ["tokenizers>=0.20"]
# ///
"""A/B retrieval eval: does storing bkmr memories as markdown hurt hsearch?

Loads the same facts (markdown_corpus.json) into throwaway bkmr databases in
four renderings and measures where the target memory ranks:

  plain         one fact per memory, prose
  md_light      one fact per memory, bullets + inline code
  md_heavy      one fact per memory, headings/tables/links/code fences
  consolidated  4 facts merged into one multi-section markdown doc per theme

Arms per query:
  hsearch  bkmr hsearch (RRF of FTS + semantic)  -> rank of target
  sem      bkmr sem-search (embedding only)      -> rank of target
  fts      bkmr search with hsearch's FTS query  -> hit / miss (FTS uses AND)

Plus a truncation probe: the same key fact stored short vs. buried after a
large markdown table, to show the 512-token embedding window cutting the tail.

Never touches the user's real DB: every call passes --db <tmpdir>/<variant>.db.

Usage:
  ./markdown_retrieval.py [--bkmr PATH] [--keep DIR] [--json OUT]
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
CORPUS = HERE / "markdown_corpus.json"
LIMIT = 100  # larger than any DB -> every memory gets a rank
VARIANTS = ["plain", "md_light", "md_heavy", "consolidated"]
EMBED_MAX_TOKENS = 512  # fastembed DEFAULT_MAX_LENGTH, silently truncates
SEM_LINE = re.compile(r"^(\d+)\t")


def run(bkmr: str, db: Path, *args: str) -> str:
    proc = subprocess.run(
        [bkmr, "--db", str(db), *args],
        capture_output=True,
        text=True,
        env={**os.environ, "NO_COLOR": "1"},
    )
    # "0 bookmarks found" exits 0; anything else nonzero is a real failure.
    if proc.returncode != 0:
        sys.exit(f"bkmr {' '.join(args)} failed ({proc.returncode}):\n{proc.stderr}")
    return proc.stdout


def build_entries(corpus: dict, variant: str) -> tuple[list[dict], dict[str, str]]:
    """Return (bookmark JSON entries, fact_id -> url of the memory holding it)."""
    entries, owner = [], {}
    if variant == "consolidated":
        for theme, theme_title in corpus["themes"].items():
            facts = [f for f in corpus["facts"] if f["theme"] == theme]
            body = f"# {theme_title}\n\n" + "\n\n".join(
                f"## {f['title']}\n\n{f['md_light']}" for f in facts
            )
            # ponytail: classification tag picked as "fact" for the merged doc;
            # a real merged doc has no single correct classification.
            topics = sorted({t for f in facts for t in f["tags"][1:]})
            entries.append(_entry(body, theme_title, ["fact", *topics]))
            for f in facts:
                owner[f["id"]] = body
    else:
        for f in corpus["facts"]:
            entries.append(_entry(f[variant], f["title"], f["tags"]))
            owner[f["id"]] = f[variant]
    for d in corpus["distractors"]:
        entries.append(_entry(d["url"], d["title"], d["tags"]))
    return entries, owner


def _entry(url: str, title: str, tags: list[str]) -> dict:
    return {"url": url, "title": title, "description": "", "tags": ["_mem_", *tags]}


def load_db(bkmr: str, workdir: Path, name: str, entries: list[dict]) -> tuple[Path, dict[str, int]]:
    db = workdir / f"{name}.db"
    src = workdir / f"{name}.json"
    src.write_text(json.dumps(entries))
    # create-db rejects --db, so it is the one call that bypasses run().
    subprocess.run([bkmr, "create-db", str(db)], capture_output=True, check=True)
    run(bkmr, db, "load-json", str(src))
    rows = json.loads(run(bkmr, db, "search", "-t", "_mem_", "--json", "--np", "-l", str(LIMIT)))
    if len(rows) != len(entries):
        sys.exit(f"{name}: loaded {len(rows)} of {len(entries)} entries")
    return db, {r["url"]: r["id"] for r in rows}


def rank_hsearch(bkmr: str, db: Path, q: str, target: int) -> int | None:
    rows = json.loads(run(bkmr, db, "hsearch", q, "-t", "_mem_", "--json", "--np", "-l", str(LIMIT)) or "[]")
    ids = [r["id"] for r in rows]
    return ids.index(target) + 1 if target in ids else None


def rank_sem(bkmr: str, db: Path, q: str, target: int) -> int | None:
    # sem-search has no --json; rows are "id\ttitle\turl\tscore". Multi-line
    # markdown urls spill over lines, so only lines starting "<digits>\t" count.
    out = run(bkmr, db, "sem-search", q, "--np", "-l", str(LIMIT))
    ids = [int(m.group(1)) for line in out.splitlines() if (m := SEM_LINE.match(line))]
    return ids.index(target) + 1 if target in ids else None


def fts_hit(bkmr: str, db: Path, q: str, target: int) -> bool:
    # Same query string hsearch hands to FTS5 (domain/search.rs fts_query).
    fts = " ".join('"' + t.replace('"', '""') + '"' for t in q.split())
    rows = json.loads(run(bkmr, db, "search", fts, "-t", "_mem_", "--json", "--np", "-l", str(LIMIT)) or "[]")
    return target in {r["id"] for r in rows}


def summarize(ranks: list[int | None]) -> dict:
    n = len(ranks)
    return {
        "mrr": round(sum(1 / r for r in ranks if r) / n, 3),
        "r@1": round(sum(1 for r in ranks if r == 1) / n, 3),
        "r@3": round(sum(1 for r in ranks if r and r <= 3) / n, 3),
        "r@5": round(sum(1 for r in ranks if r and r <= 5) / n, 3),
        "median_rank": statistics.median(r or LIMIT for r in ranks),
    }


def embedding_text(tags: list[str], title: str, content: str) -> str:
    """Mirror of domain/bookmark.rs build_embedding_content (+ Nomic doc prefix)."""
    visible = sorted(t for t in tags if not (t.startswith("_") or t.endswith("_")))
    tag_str = "," + ",".join(visible) + "," if visible else ""
    return f"search_document: {tag_str}{title} -- {content}{tag_str}"


def load_tokenizer():
    from tokenizers import Tokenizer

    cache = os.environ.get("FASTEMBED_CACHE_DIR", str(Path.home() / "Library/Caches/bkmr/models"))
    hits = glob.glob(f"{cache}/models--nomic-ai--nomic-embed-text-v1.5/snapshots/*/tokenizer.json")
    if not hits:
        sys.exit(f"Nomic tokenizer.json not found under {cache}; run any bkmr embed once first")
    return Tokenizer.from_file(hits[0])


def token_stats(corpus: dict, tok) -> dict:
    out = {}
    for v in ["plain", "md_light", "md_heavy"]:
        counts = [len(tok.encode(embedding_text(f["tags"], f["title"], f[v])).ids) for f in corpus["facts"]]
        out[v] = {"mean": round(statistics.mean(counts)), "max": max(counts), "over_512": sum(c > EMBED_MAX_TOKENS for c in counts)}
    return out


def truncation_probe(bkmr: str, workdir: Path, corpus: dict, tok) -> dict:
    p = corpus["truncation_probe"]
    rows = "\n".join(f"| event_{i} | `/hooks/v1/evt{i}` | retry {i % 5} | {i * 10}ms |" for i in range(p["filler_rows"]))
    buried = f"## Webhook endpoints\n\n| Event | Path | Retry | Timeout |\n|---|---|---|---|\n{rows}\n\n## Signature\n\n{p['key_fact']}"
    results = {}
    for name, content in [("short", p["key_fact"]), ("buried", buried)]:
        entries, _ = build_entries(corpus, "plain")
        entries.append(_entry(content, p["title"], p["tags"]))
        db, ids = load_db(bkmr, workdir, f"probe_{name}", entries)
        target = ids[content]
        text = embedding_text(p["tags"], p["title"], content)
        n_tok = len(tok.encode(text).ids)
        results[name] = {
            "tokens": n_tok,
            "truncated": n_tok > EMBED_MAX_TOKENS,
            "sem_rank": rank_sem(bkmr, db, p["query"], target),
            "hsearch_rank": rank_hsearch(bkmr, db, p["query"], target),
        }
    return results


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bkmr", default=shutil.which("bkmr") or "bkmr")
    ap.add_argument("--keep", type=Path, help="keep DBs in this dir instead of a tempdir")
    ap.add_argument("--json", type=Path, help="write full per-query results here")
    args = ap.parse_args()

    corpus = json.loads(CORPUS.read_text())
    tok = load_tokenizer()
    workdir = args.keep or Path(tempfile.mkdtemp(prefix="bkmr-md-eval-"))
    workdir.mkdir(parents=True, exist_ok=True)

    per_query: dict[str, list[dict]] = {}
    for v in VARIANTS:
        print(f"[{v}] loading + embedding ...", file=sys.stderr)
        entries, owner = build_entries(corpus, v)
        db, ids = load_db(args.bkmr, workdir, v, entries)
        rows = []
        for q in corpus["queries"]:
            target = ids[owner[q["target"]]]
            rows.append({
                **q,
                "hsearch": rank_hsearch(args.bkmr, db, q["q"], target),
                "sem": rank_sem(args.bkmr, db, q["q"], target),
                "fts_hit": fts_hit(args.bkmr, db, q["q"], target),
            })
        per_query[v] = rows

    report = {"summary": {}, "tokens": token_stats(corpus, tok), "truncation_probe": truncation_probe(args.bkmr, workdir, corpus, tok)}
    for v, rows in per_query.items():
        report["summary"][v] = {}
        for kind in ["all", "kw", "para"]:
            sel = [r for r in rows if kind == "all" or r["kind"] == kind]
            report["summary"][v][kind] = {
                "n": len(sel),
                "hsearch": summarize([r["hsearch"] for r in sel]),
                "sem": summarize([r["sem"] for r in sel]),
                "fts_hit_rate": round(sum(r["fts_hit"] for r in sel) / len(sel), 3),
            }

    print_report(report, corpus)
    if args.json:
        args.json.write_text(json.dumps({**report, "per_query": per_query}, indent=2))
    if not args.keep:
        shutil.rmtree(workdir, ignore_errors=True)


def print_report(report: dict, corpus: dict) -> None:
    n_docs = len(corpus["facts"]) + len(corpus["distractors"])
    print(f"\nCorpus: {len(corpus['facts'])} target facts, {len(corpus['distractors'])} distractors (~{n_docs} memories/DB), {len(corpus['queries'])} queries\n")
    for kind in ["all", "kw", "para"]:
        print(f"== queries: {kind} ==")
        print(f"{'variant':<13} {'hs MRR':>7} {'hs R@1':>7} {'hs R@5':>7} {'sem MRR':>8} {'sem R@1':>8} {'sem R@5':>8} {'FTS hit':>8}")
        for v, s in report["summary"].items():
            k = s[kind]
            print(f"{v:<13} {k['hsearch']['mrr']:>7} {k['hsearch']['r@1']:>7} {k['hsearch']['r@5']:>7} "
                  f"{k['sem']['mrr']:>8} {k['sem']['r@1']:>8} {k['sem']['r@5']:>8} {k['fts_hit_rate']:>8}")
        print()
    print("== embedding input tokens (Nomic tokenizer, incl. prefix/title/tags) ==")
    for v, t in report["tokens"].items():
        print(f"{v:<13} mean {t['mean']:>4}  max {t['max']:>4}  over 512: {t['over_512']}")
    print("\n== truncation probe ==")
    for name, r in report["truncation_probe"].items():
        print(f"{name:<7} tokens {r['tokens']:>5} truncated={r['truncated']!s:<5} sem rank {r['sem_rank']}  hsearch rank {r['hsearch_rank']}")


if __name__ == "__main__":
    main()
