# bkmr

# Bookmark Manager and Launcher

[sysid blog: twbm](https://sysid.github.io/bkmr/)

Features:
- manages URIs in sqlite database
- full-text search across URIs with semantic ranking (FTS5)
- fuzzy search (similar to FZF)
- tags for URI classification
- knows how to open HTTP URLs, directories, files (e.g. Office, Images, ....)
- can execute URIs as shell commands via the protocol prefix: 'shell::'
  URI-Example: `shell::vim +/"## SqlAlchemy" $HOME/document.md`
- enriches added URL with title and description from Web

To harness `bkmr`'s power use full-text query syntax (see: https://www.sqlite.org/fts5.html chapter 3).

## Usage
```bash
bkmr --help

A bookmark manager for the terminal

Usage: bkmr [OPTIONS] [NAME] [COMMAND]

Commands:
  search     Searches Bookmarks
  open       Open/launch bookmarks
  add        add a bookmark
  delete     Delete bookmarks
  update     Update bookmarks
  edit       Edit bookmarks
  show       Show Bookmarks (list of ids, separated by comma, no blanks)
  tags       tag for which related tags should be shown. No input: all tags are printed
  create-db  Initialize bookmark database
  help       Print this message or the help of the given subcommand(s)

Arguments:
  [NAME]  Optional name to operate on

Options:
  -c, --config <FILE>  Sets a custom config file
  -d, --debug...       Turn debugging information on
  -h, --help           Print help information
  -V, --version        Print version information
```
<a href="https://asciinema.org/a/rDjGHV4LC8k9yuLTIhlGS1rv0?autoplay=1&speed=2"><img src="https://asciinema.org/a/rDjGHV4LC8k9yuLTIhlGS1rv0.png" width="836"/></a>

### Examples
```bash
# FTS examples (https://www.sqlite.org/fts5.htm)
bkmr search 'security "single-page"'
bkmr search '"https://securit" *'
bkmr search '^security'
bkmr search 'postgres OR sqlite'
bkmr search 'security NOT keycloak'

# FTS combined with tag filtering
bkmr search -t tag1,tag2 -n notag1 <searchquery>

# Match exact taglist
bkmr search -e tag1,tag2

# Search by any tag and sort by bookmark age ascending
bkmr search -T tag1,tag2 -O

# Adding URI to local files
bkmr add /home/user/presentation.pptx tag1,tag2 --title 'My super Presentation'

# Adding shell commands as URI
bkmr add "shell::vim +/'# SqlAlchemy' sql.md" shell,sql,doc --title 'sqlalchemy snippets'
```
Tags must be separated by comma without blanks.


## Installation
1. standard Rust install via `cargo`
2. initialize the database: `bkmr create-db db_path`


## Configuration
Location of sqlite database must be known:
```bash
export "BKMR_DB_URL=db-path"
```

## Benchmarking
- -20x faster than the original after warming up Python.
```bash
time twbm search 'zzz*' --np
0. zzzeek : Asynchronous Python and Databases [345]
   https://techspot.zzzeek.org/2015/02/15/asynchronous-python-and-databases/
   async, knowhow, py

Found: 1
345

real    0m0.259s
user    0m0.220s
sys     0m0.037s



time bmkr search 'zzz*' --np
-bash: bmkr: command not found

real    0m0.014s
user    0m0.005s
sys     0m0.009s
```

- panics on null values in DB, but there should'nt be any
- script provided for finding and cleaning

<!-- Badges -->
[pypi-image]: https://img.shields.io/pypi/v/bkmr?color=blue
[pypi-url]: https://pypi.org/project/bkmr/
[build-image]: https://github.com/sysid/bkmr/actions/workflows/build.yml/badge.svg
[build-url]: https://github.com/sysid/bkmr/actions/workflows/build.yml
[coverage-image]: https://codecov.io/gh/sysid/bkmr/branch/main/graph/badge.svg
[coverage-url]: https://codecov.io/gh/sysid/bkmr
[quality-image]: https://api.codeclimate.com/v1/badges/3130fa0ba3b7993fbf0a/maintainability
[quality-url]: https://codeclimate.com/github/nalgeon/podsearch-py
