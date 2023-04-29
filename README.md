# bkmr

# Ultrafast Bookmark Manager and Launcher

Features:
- full-text search with semantic ranking (FTS5)
- fuzzy search `--fzf` (CTRL-O: copy to clipboard, CTRL-E: edit, Enter: open)
- tags for classification
- knows how to open HTTP URLs, directories, files (e.g. Office, Images, ....)
- can execute URI strings as shell commands via protocol prefix: 'shell::'
  URI-Example: `shell::vim +/"## SqlAlchemy" $HOME/document.md`
- automatically enriches URLs with title and description from Web

> `bkmr search --fzf` is a great way to quickly find and open bookmarks.

To fully use `bkmr`'s full-text query power see: https://www.sqlite.org/fts5.html (chapter 3).

## Usage
```bash
bkmr --help

A bookmark manager for the terminal

Usage: bkmr [OPTIONS] [NAME] [COMMAND]

Commands:
  search     Searches Bookmarks
  open       Open/launch bookmarks
  add        Add a bookmark
  delete     Delete bookmarks
  update     Update bookmarks
  edit       Edit bookmarks
  show       Show Bookmarks (list of ids, separated by comma, no blanks)
  tags       Tag for which related tags should be shown. No input: all tags are printed
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

<a href="https://asciinema.org/a/ULCDIrw4pG9diaVJb17AjIAa7?autoplay=1&speed=2"><img src="https://asciinema.org/a/ULCDIrw4pG9diaVJb17AjIAa7.png" width="836"/></a>

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

# Bulk updating bookmarks: all bms with tag 'sa' also get tag 'py'
bkmr update -t py $(bkmr search -t sa --np)
# Bulk updating bookmarks: all bms with tag 'gh' should have 'git' removed
bkmr update -n git $(bkmr search -t gh --np)

# JSON dump of entire database
bkmr search --json
```
Tags must be separated by comma without blanks.


## Installation
1. `cargo install bkmr`
2. initialize the database: `bkmr create-db db_path`
3. add URLs


### Configuration
Location of created sqlite database must be known:
```bash
export "BKMR_DB_URL=db-path"
```

You can set (not required) this variable to change `--fzf` mode default options:
```bash
export "BKMR_FZF_OPTS=--reverse --height 20% --show-tags"
```
Currently available flags: 
- `--reverse` (defaults to false)
- `--height` (defaults to 50%)
- ``--show-tags`` (defaults to false)

## Benchmarking
- ca. 20x faster than the Python original [twbm](https://github.com/sysid/twbm) after warming up Python.
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
[pypi-image]: https://img.shields.io/pypi/v/bkmr?color=blue
[pypi-url]: https://pypi.org/project/bkmr/
[build-image]: https://github.com/sysid/bkmr/actions/workflows/build.yml/badge.svg
[build-url]: https://github.com/sysid/bkmr/actions/workflows/build.yml
[coverage-image]: https://codecov.io/gh/sysid/bkmr/branch/main/graph/badge.svg
[coverage-url]: https://codecov.io/gh/sysid/bkmr
[quality-image]: https://api.codeclimate.com/v1/badges/3130fa0ba3b7993fbf0a/maintainability
[quality-url]: https://codeclimate.com/github/nalgeon/podsearch-py
