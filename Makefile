.DEFAULT_GOAL := help
#MAKEFLAGS += --no-print-directory

# You can set these variables from the command line, and also from the environment for the first two.
PREFIX ?= /usr/local
BINPREFIX ?= "$(PREFIX)/bin"

VERSION       = $(shell cat VERSION)

SHELL	= bash
.ONESHELL:

app_root := $(if $(PROJ_DIR),$(PROJ_DIR),$(CURDIR))

pkg_src =  $(app_root)/bkmr
tests_src = $(app_root)/bkmr/tests
BINARY = bkmr

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

.PHONY: init-demo
init-demo:  ## init-demo
	@rm -fr ~/xxx
	mkdir -p ~/xxx/bkmr-demos
	bkmr create-db ~/xxx/bkmr-demos/demo.db
	@tree -a ~/xxx

.PHONY: init
init:  ## init
	@rm -vf $(app_root)/db/*.db
	@rm -fr ~/xxx
	mkdir -p ~/xxx
	@echo "-M- copy full buku db to ~/xxx"
	@cp -v $(VIMWIKI_PATH)/buku/bm.db ~/xxx/bkmr.db
	@cp -vf bkmr/tests/resources/bkmr.v?.db ~/xxx/
	@cp -vf bkmr/tests/resources/bkmr.v2.db $(app_root)/db/bkmr.db
	@tree -a ~/xxx
	@tree -a  $(app_root)/db

.PHONY: test
test:  ## tests, single-threaded
	RUST_LOG=skim=info BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo test -- --test-threads=1

.PHONY: run-all
#run-all: test-url-details test-env run-migrate-db run-backfill run-update run-show run-create-db run-edit-sem run-tags run-delete run-add run-search ## run-all
run-all: run-migrate-db run-backfill run-update run-show run-create-db run-edit-sem run-tags run-delete run-add run-search  ## run-all

.PHONY: test-edit-bookmark-with-template
test-edit-bookmark-with-template: init  ## test-edit-bookmark-with-template (file should be updated)
	RUST_LOG=skim=info BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo test --package bkmr --lib -- application::services::template_service::tests::test_edit_bookmark_with_template --ignored --nocapture --exact

.PHONY: test-url-details
test-url-details:  ## test-url-details (charm strang verbose output), expect: "Rust Programming Language", "A language empowering everyone to build reliable and efficient software."
	RUST_LOG=skim=info BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo test --package bkmr --test test_lib given_valid_url_when_loading_details_then_returns_correct_metadata -- --exact --nocapture

.PHONY: run-load-texts
run-load-texts: run-create-db  ## run-load-text
	pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test.db cargo run -- -d -d --openai load-texts --dry-run "$(PROJ_DIR)"/bkmr/tests/resources/data.ndjson
	#pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test.db cargo run -- -d -d --openai load-texts "$(PROJ_DIR)"/bkmr/tests/resources/data.ndjson


.PHONY: run-migrate-db
run-migrate-db: init  ## run-migrate-db
	@echo "--------------------------------------------------------------------------------"
	@echo "-M- First run: should do migration"
	pushd $(pkg_src) && BKMR_DB_URL=$(HOME)/xxx/bkmr.v1.db cargo run -- -d -d -d --openai
	@echo "--------------------------------------------------------------------------------"
	@echo "-M- Second run: should be ok, do nothing"
	pushd $(pkg_src) && BKMR_DB_URL=$(HOME)/xxx/bkmr.v1.db cargo run -- -d -d -d --openai

.PHONY: run-backfill
run-backfill: run-create-db  ## run-backfill
	pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test.db cargo run -- -d -d --openai backfill --dry-run  # only shows "Google" entry


.PHONY: run-create-db
run-create-db:  ## run-create-db: opens new /tmp/bkmr_test.db
	rm -vf /tmp/bkmr_test.db
	pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test_db cargo run -- -d -d create-db /tmp/bkmr_test.db
	open /tmp/bkmr_test.db

.PHONY: run-edit-sem
run-edit-sem: init  ## run-edit-sem with openai semantic
	pushd $(pkg_src) && BKMR_DB_URL=~/xxx//bkmr.v2.db cargo run -- -d -d --openai edit 1

.PHONY: run-edit
run-edit: init-db   ## run-edit v1
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d edit 1,3

.PHONY: install-diesel-cli
install-diesel-cli:  ## install-diesel-cli
	cargo install diesel_cli --no-default-features --features sqlite
	asdf reshim rust

################################################################################
# Building, Deploying \
BUILDING:  ## ##################################################################

.PHONY: all
all: clean build install  ## all
	:

.PHONY: all-fast
all-fast: clean build-fast install-debug  ## all-fast: no release build
	:

.PHONY: generate-ci
generate-ci:  ## generate-ci
	maturin generate-ci github --platform macos --platform linux -m bkmr/Cargo.toml

.PHONY: upload
upload:  ## upload
	@if [ -z "$$CARGO_REGISTRY_TOKEN" ]; then \
		echo "Error: CARGO_REGISTRY_TOKEN is not set"; \
		exit 1; \
	fi
	@echo "CARGO_REGISTRY_TOKEN is set"
	pushd $(pkg_src) && cargo release publish --execute

.PHONY: build-wheel
build-wheel:  ## build-wheel
	maturin build --release -m bkmr/Cargo.toml

.PHONY: build
build:  ## build
	pushd $(pkg_src) && cargo build --release

.PHONY: build-fast
build-fast:  ## build-fast
	pushd $(pkg_src) && cargo build

.PHONY: install-debug
install-debug: uninstall  ## install-debug (no release version)
	@VERSION=$(shell cat VERSION) && \
		echo "-M- Installing $$VERSION" && \
		cp -vf bkmr/target/debug/$(BINARY) ~/bin/$(BINARY)$$VERSION && \
		ln -vsf ~/bin/$(BINARY)$$VERSION ~/bin/$(BINARY)
		~/bin/$(BINARY) completion bash > ~/.bash_completions/bkmr

.PHONY: install
install: uninstall  ## install
	@VERSION=$(shell cat VERSION) && \
		echo "-M- Installing $$VERSION" && \
		cp -vf bkmr/target/release/$(BINARY) ~/bin/$(BINARY)$$VERSION && \
		ln -vsf ~/bin/$(BINARY)$$VERSION ~/bin/$(BINARY)
		~/bin/$(BINARY) completion bash > ~/.bash_completions/bkmr

.PHONY: uninstall
uninstall:  ## uninstall
	-@test -f ~/bin/$(BINARY) && rm -v ~/bin/$(BINARY)
	rm -vf ~/.bash_completions/bkmr

.PHONY: bump-major
bump-major:  check-github-token  ## bump-major, tag and push
	bump-my-version bump --commit --tag major
	git push
	git push --tags
	@$(MAKE) create-release

.PHONY: bump-minor
bump-minor:  check-github-token  ## bump-minor, tag and push
	bump-my-version bump --commit --tag minor
	git push
	git push --tags
	@$(MAKE) create-release

.PHONY: bump-patch
bump-patch:  check-github-token  ## bump-patch, tag and push
	bump-my-version bump --commit --tag patch
	git push
	git push --tags
	@$(MAKE) create-release

.PHONY: create-release
create-release: check-github-token  ## create a release on GitHub via the gh cli
	@if ! command -v gh &>/dev/null; then \
		echo "You do not have the GitHub CLI (gh) installed. Please create the release manually."; \
		exit 1; \
	else \
		echo "Creating GitHub release for v$(VERSION)"; \
		gh release create "v$(VERSION)" --generate-notes --latest; \
	fi

.PHONY: check-github-token
check-github-token:  ## Check if GITHUB_TOKEN is set
	@if [ -z "$$GITHUB_TOKEN" ]; then \
		echo "GITHUB_TOKEN is not set. Please export your GitHub token before running this command."; \
		exit 1; \
	fi
	@echo "GITHUB_TOKEN is set"
	#@$(MAKE) fix-version  # not working: rustrover deleay


.PHONY: fix-version
fix-version:  ## fix-version of Cargo.toml, re-connect with HEAD
	git add bkmr/Cargo.lock
	git commit --amend --no-edit
	git tag -f "v$(VERSION)"
	git push --force-with-lease
	git push --tags --force

.PHONY: format
format:  ## format
	BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo fmt

.PHONY: lint
lint:  ## lint and fix
	#BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo clippy
	BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo clippy --fix  -- -A unused_imports  # avoid errors
	BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo fix --lib -p bkmr --tests

.PHONY: doc
doc:  ## doc
	@rustup doc --std
	pushd $(pkg_src) && cargo doc --open

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
	pushd $(pkg_src) && cargo clean -v

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
