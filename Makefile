.DEFAULT_GOAL := help
#MAKEFLAGS += --no-print-directory

# You can set these variables from the command line, and also from the environment for the first two.
PREFIX ?= /usr/local
BINPREFIX ?= "$(PREFIX)/bin"

VERSION       = $(shell cat VERSION)

SHELL	= bash
.ONESHELL:

app_root = .
app_root ?= .
pkg_src =  $(app_root)/twbm
tests_src = $(app_root)/tests

# Makefile directory
CODE_DIR := $(dir $(abspath $(lastword $(MAKEFILE_LIST))))

# define files
MANS = $(wildcard ./*.md)
MAN_HTML = $(MANS:.md=.html)
MAN_PAGES = $(MANS:.md=.1)
# avoid circular targets
MAN_BINS = $(filter-out ./tw-extras.md, $(MANS))

################################################################################
# Admin \
ADMIN::  ## ##################################################################

.PHONY: test-open-uri-url
test-open-uri-url:  ## test-open-uri-url
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo test --package twbm --lib process::test::test_open_bm::case_1 -- --nocapture

.PHONY: test-open-uri-pptx
test-open-uri-pptx:  ## test-open-uri-pptx
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo test --package twbm --lib process::test::test_open_bm::case_2 -- --nocapture

.PHONY: test-open-uri-vim
test-open-uri-vim:  ## test-open-uri-vim
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo test --package twbm --lib process::test::test_open_bm::case_3 -- --nocapture

.PHONY: test-open-uri-all
test-open-uri-all: test-open-uri-vim test-open-uri-pptx test-open-uri-url  ## test-open-uri all

.PHONY: run-show
run-show: init-db  ## run-show
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo run -- -d -d show 1,10

.PHONY: run-init-db
run-init-db:  ## run-init-db
	test -f /tmp/rtwbm_test.db && rm -v /tmp/rtwbm_test.db
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo run -- -d -d create-db /tmp/rtwbm_test.db
	open /tmp/rtwbm_test.db

.PHONY: run-edit
run-edit: init-db   ## run-edit
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo run -- -d -d edit 1,3

.PHONY: run-tags
run-tags: init-db  ## run-tags
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo run -- -d -d tags bbb
	@echo "------ all tags -----"
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo run -- -d -d tags

.PHONY: run-delete
run-delete: init-db  ## run-delete
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo run -- -d -d delete 1,2,3

.PHONY: run-add
run-add: init-db  ## run-add
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo run -- -d -d add sysid_new_url t1,t2 --title 'sysid New URL title'

.PHONY: run-search
run-search: init-db  ## run-search interactively for manual tests
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo run -- -d -d search

.PHONY: init-db
init-db:  ## init-db
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo test --package twbm --test test_lib test_dal::test_init_db -- --exact

.PHONY: install-diesel-cli
install-diesel-cli:  ## install-diesel-cli
	cargo install diesel_cli --no-default-features --features sqlite
	asdf reshim rust

.PHONY: test-vim
test-vim:  ## test-vim
	#pushd twbm && cargo test --color=always --package twbm --lib process::test::test_do_edit -- --nocapture --ignored
	RTWBM_DB_URL=../db/twbm.db pushd twbm && cargo test --color=always --test test_process test_do_edit -- --nocapture --ignored

.PHONY: test-dal
test-dal:  ## test-dal
	RTWBM_DB_URL=../db/twbm.db RUST_LOG=DEBUG pushd twbm && cargo test --package twbm --test test_lib "" -- --test-threads=1

.PHONY: test
test:  test-dal  ## test (must run DB test before to init ?!?)
	RTWBM_DB_URL=../db/twbm.db RUST_LOG=DEBUG pushd twbm && cargo test --package twbm -- --test-threads=1 # --nocapture

.PHONY: benchmark
benchmark:  ## benchmark
	time RTWBM_DB_URL=/Users/Q187392/dev/s/private/vimwiki/buku/bm.db_20230110_170737 /Users/Q187392/dev/s/private/rs-twbm/twbm/target/release/twbm search zzzeek --np
	@echo "-----------------------------------------------------------"
	time TWBM_DB_URL=sqlite://///Users/Q187392/dev/s/private/vimwiki/buku/bm.db_20230110_170737 /Users/Q187392/.local/bin/twbm search zzzeek --np


################################################################################
# Building, Deploying \
BUILDING:  ## ##################################################################

.PHONY: all
all: clean build install  ## all
	:

.PHONY: build
build:  ## build
	pushd twbm && cargo build --release

.PHONY: install
install:  ## install
	#pushd twbm && cargo install --path . --root ~/.cargo
	@cp -vf twbm/target/release/twbm ~/bin/rtwbm

.PHONY: uninstall
uninstall:  ## uninstall
	#pushd twbm && cargo uninstall --root ~/.cargo
	@test -f ~/bin/rtwbm && rm -v ~/bin/rtwbm

.PHONY: bump-major
bump-major:  ## bump-major, tag and push
	bumpversion --commit --tag major
	git push --tags

.PHONY: bump-minor
bump-minor:  ## bump-minor, tag and push
	bumpversion --commit --tag minor
	git push --tags

.PHONY: bump-patch
bump-patch:  ## bump-patch, tag and push
	bumpversion --commit --tag patch
	git push --tags

################################################################################
# Clean \
CLEAN:  ## ############################################################

.PHONY: clean
clean:clean-rs  ## clean all
	:

.PHONY: clean-build
clean-build: ## remove build artifacts
	rm -fr build/
	rm -fr dist/
	rm -fr .eggs/
	find . \( -path ./env -o -path ./venv -o -path ./.env -o -path ./.venv \) -prune -o -name '*.egg-info' -exec rm -fr {} +
	find . \( -path ./env -o -path ./venv -o -path ./.env -o -path ./.venv \) -prune -o -name '*.egg' -exec rm -f {} +

.PHONY: clean-pyc
clean-pyc: ## remove Python file artifacts
	find . -name '*.pyc' -exec rm -f {} +
	find . -name '*.pyo' -exec rm -f {} +
	find . -name '*~' -exec rm -f {} +
	find . -name '__pycache__' -exec rm -fr {} +

.PHONY: clean-rs
clean-rs:  ## clean-rs
	pushd twbm && cargo clean -v

################################################################################
# Misc \
MISC:  ## ############################################################

define PRINT_HELP_PYSCRIPT
import re, sys

for line in sys.stdin:
	match = re.match(r'^([%a-zA-Z0-9_-]+):.*?## (.*)$$', line)
	if match:
		target, help = match.groups()
		if target != "dummy":
			print("\033[36m%-20s\033[0m %s" % (target, help))
endef
export PRINT_HELP_PYSCRIPT

.PHONY: help
help:
	@python -c "$$PRINT_HELP_PYSCRIPT" < $(MAKEFILE_LIST)

debug:  ## debug
	@echo "-D- CODE_DIR: $(CODE_DIR)"


.PHONY: list
list: *  ## list
	@echo $^

.PHONY: list2
%: %.md  ## list2
	@echo $^


%-plan:  ## call with: make <whatever>-plan
	@echo $@ : $*
	@echo $@ : $^
