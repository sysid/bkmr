# bkmr

![Crates.io](https://img.shields.io/crates/v/bkmr)
![Crates.io](https://img.shields.io/crates/d/bkmr)
[![Docs.rs](https://docs.rs/bkmr/badge.svg)](https://docs.rs/bkmr)

[![PyPI Version][pypi-image]][pypi-url]
[![Downloads](https://static.pepy.tech/badge/bkmr/month)](https://pepy.tech/project/bkmr)
[![Build Status][build-image]][build-url]


### [Generalized Semantic Search](https://github.com/sysid/bkmr/wiki/Semantic-Search)

# Ultrafast Bookmark Manager and Launcher

> New Feature: Semantic Search (AI Embeddings)

[Elevating Bookmark Management with AI-Driven Semantic Search](https://sysid.github.io/elevating-bookmark-management-with-ai-driven-semantic-search/)

Features:
- semantic search using OpenAI embeddings (requires OpenAI API key)
- full-text search with semantic ranking (FTS5)
- fuzzy search `--fzf` (CTRL-O: copy to clipboard, CTRL-E: edit, CTRL-D: delete, Enter: open)
- tags for classification
- can handle HTTP URLs, directories, files (e.g. Office, Images, ....)
- can execute URI strings as shell commands via protocol prefix: 'shell::'
  URI-Example: `shell::vim +/"## SqlAlchemy" $HOME/document.md`
- automatically enriches URLs with title and description from Web
- manages statistics about bookmark usage

**`bkmr search --fzf` is a great way to open bookmarks very fast.**

## Usage
```bash
bkmr --help

A Bookmark Manager and Launcher for the Terminal

Usage: bkmr [OPTIONS] [NAME] [COMMAND]

Commands:
  search      Searches Bookmarks
  sem-search  Semantic Search with OpenAI
  open        Open/launch bookmarks
  add         Add a bookmark
  delete      Delete bookmarks
  update      Update bookmarks
  edit        Edit bookmarks
  show        Show Bookmarks (list of ids, separated by comma, no blanks)
  surprise    Opens n random URLs
  tags        Tag for which related tags should be shown. No input: all tags are printed
  create-db   Initialize bookmark database
  backfill    Backfill embeddings for bookmarks
  load-texts  Load texts for semantic similarity search
  help        Print this message or the help of the given subcommand(s)

Arguments:
  [NAME]  Optional name to operate on
```

<a href="https://asciinema.org/a/ULCDIrw4pG9diaVJb17AjIAa7?autoplay=1&speed=2"><img src="https://asciinema.org/a/ULCDIrw4pG9diaVJb17AjIAa7.png" width="836"/></a>

### Examples
```bash
# FTS examples (https://www.sqlite.org/fts5.htm)
bkmr search '"https://securit" *'
bkmr search 'security NOT keycloak'

# FTS combined with tag filtering
bkmr search -t tag1,tag2 -n notag1 <searchquery>

# Search by any tag and sort by bookmark age ascending
bkmr search -T tag1,tag2 -O

# Give me the 10 oldest bookmarks
bkmr search -O --limit 10

# Adding URI to local files
bkmr add /home/user/presentation.pptx tag1,tag2 --title 'My super Presentation'

# Adding shell commands as URI
bkmr add "shell::vim +/'# SqlAlchemy' sql.md" shell,sql,doc --title 'sqlalchemy snippets'

# JSON dump of entire database
bkmr search --json

# Semantic Search based on OpenAI Embeddings
bkmr --openai sem-search "python security"  # requires OPENAI_API_KEY
```
Tags must be separated by comma without blanks.

## Installation
1. `cargo install bkmr`
2. initialize the database: `bkmr create-db db_path`
3. `export "BKMR_DB_URL=db-path"`, location of created sqlite database must be known
4. add URLs

If you do not have Rust on your machine you can use: `pip install bkmr`

More configuration options can be found at [documentation page](https://github.com/sysid/bkmr/wiki/configuration).

### Upgrade to 1.x.x
A database migration will be performed on the first run of the new version.
This will add two columns to the bookmarks table for the OpenAI embeddings.
No destructive changes are made to the database.

## Semantic Search
`bkmr` provides now full semantic search of generalized bookmarks using OpenAI's Embeddings. 

You can find more information on the [documentation page](https://github.com/sysid/bkmr/wiki/semantic-search).

## Benchmarking
- ca. 20x faster than the Python original [twbm](https://github.com/sysid/twbm) after warming up Python.
- same for [buku](https://github.com/jarun/buku).
```bash
time twbm search 'zzz*' --np
0. zzzeek : Asynchronous Python and Databases [343]
   https://techspot.zzzeek.org/2015/02/15/asynchronous-python-and-databases/
   async, knowhow, py


Found: 1
343

real    0m0.501s
user    0m0.268s
sys     0m0.070s



time bkmr search 'zzz*' --np
1. zzzeek : Asynchronous Python and Databases [343]
   https://techspot.zzzeek.org/2015/02/15/asynchronous-python-and-databases/
   async knowhow py


real    0m0.027s
user    0m0.008s
sys     0m0.016s
```
[sysid blog: bkmr](https://sysid.github.io/bkmr/)


<!-- Badges -->
[pypi-url]: https://pypi.org/project/bkmr/

[build-image]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml/badge.svg
[build-url]: https://github.com/sysid/bkmr/actions/workflows/release_wheels.yml

[quality-image]: https://api.codeclimate.com/v1/badges/3130fa0ba3b7993fbf0a/maintainability
[quality-url]: https://codeclimate.com/github/nalgeon/podsearch-py

[pypi-image]: https://badge.fury.io/py/bkmr.svg
[pypi-url]: https://pypi.org/project/bkmr/
