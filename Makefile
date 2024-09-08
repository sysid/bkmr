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

.PHONY: init
init:  ## init
	rm -vf $(app_root)/db/*.db
	rm -fr ~/xxx
	mkdir -p ~/xxx
	echo "-M- copy full buku db to ~/xxx"
	cp -v $(VIMWIKI_PATH)/buku/bm.db ~/xxx/bkmr.db
	cp -vf bkmr/tests/resources/bkmr.v?.db ~/xxx/
	tree -a ~/xxx
	tree -a  $(app_root)/db

.PHONY: run-all
#run-all: test-url-details test-env run-migrate-db run-backfill run-update run-show run-create-db run-edit-sem run-tags run-delete run-add run-search ## run-all
run-all: test-env run-migrate-db run-backfill run-update run-show run-create-db run-edit-sem run-tags run-delete run-add run-search  ## run-all


.PHONY: test-url-details
test-url-details:  ## test-url-details (charm strang verbose output)
	RUST_LOG=skim=info BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo test --package bkmr --test test_lib test_load_url_details -- --exact --nocapture

.PHONY: test-fzf  # TODO: fix
test-fzf:  ## test-fzf
	# requires to uncomment associated test
	export "BKMR_FZF_OPTS=--reverse --height 20% --show-tags" && RUST_LOG=skim=info pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo test --package bkmr --test test_fzf test_fzf -- --exact --nocapture --ignored
	#RUST_LOG=skim=info BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo test --package bkmr --test test_fzf test_fzf -- --exact --nocapture --ignored

.PHONY: test-open-uri-url
test-open-uri-url:  ## test-open-uri-url
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo test --package bkmr --lib process::test::test_open_bm::case_1 -- --nocapture

.PHONY: test-open-uri-pptx
test-open-uri-pptx:  ## test-open-uri-pptx
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo test --package bkmr --lib process::test::test_open_bm::case_2 -- --nocapture

.PHONY: test-open-uri-vim
test-open-uri-vim:  ## test-open-uri-vim
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo test --package bkmr --lib process::test::test_open_bm::case_3 -- --nocapture

.PHONY: test-open-uri-all
test-open-uri-all: test-open-uri-vim test-open-uri-pptx test-open-uri-url  ## test-open-uri all

.PHONY: test-env
test-env:  ## test-env
	#export BKMR_DB_URL=../db/bkmr.db && pushd $(pkg_src) && cargo test --package bkmr --lib environment::test -- --nocapture
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo test --package bkmr --lib environment::test -- --nocapture

.PHONY: run-load-texts
run-load-texts: run-create-db  ## run-load-text
	pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test.db cargo run -- -d -d --openai load-texts --dry-run "$(PROJ_DIR)"/bkmr/tests/resources/data.ndjson
	#pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test.db cargo run -- -d -d --openai load-texts "$(PROJ_DIR)"/bkmr/tests/resources/data.ndjson


.PHONY: run-migrate-db
run-migrate-db: init  ## run-migrate-db
	echo "-M- First run: should do migration"
	pushd $(pkg_src) && BKMR_DB_URL=$(HOME)/xxx/bkmr.v1.db cargo run -- -d -d --openai
	echo "-M- Second run: should be ok, do nothing"
	pushd $(pkg_src) && BKMR_DB_URL=$(HOME)/xxx/bkmr.v1.db cargo run -- -d -d --openai

.PHONY: run-backfill
run-backfill: run-create-db  ## run-backfill
	#cp -vf bkmr/tests/resources/bkmr.v2.db db/bkmr.v2.db
	#pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test.db cargo run -- -d -d --openai backfill
	pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test.db cargo run -- -d -d --openai backfill --dry-run  # only shows "Google" entry


.PHONY: run-update
run-update: init-db  ## run-update
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d update 1 --tags t1,t2 --ntags xxx

.PHONY: run-show
run-show: init-db  ## run-show
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d show 1,10

.PHONY: run-create-db
run-create-db:  ## run-create-db
	rm -vf /tmp/bkmr_test.db
	pushd $(pkg_src) && BKMR_DB_URL=/tmp/bkmr_test_db cargo run -- -d -d create-db /tmp/bkmr_test.db
	open /tmp/bkmr_test.db

.PHONY: run-edit-sem
run-edit-sem: init  ## run-edit-sem with openai semantic
	pushd $(pkg_src) && BKMR_DB_URL=~/xxx//bkmr.v2.db cargo run -- -d -d --openai edit 1

.PHONY: run-edit
run-edit: init-db   ## run-edit v1
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d edit 1,3

.PHONY: run-tags
run-tags: init-db  ## run-tags
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d tags bbb
	@echo "------ all tags -----"
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d tags

.PHONY: run-delete
run-delete: init-db  ## run-delete
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d delete 1,2,3

.PHONY: run-add
run-add: init-db  ## run-add
	#BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo run -- -d -d add sysid_new_url t1,t2 --title 'sysid New URL title'  # should add bespoke URI
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d add https://www.rust-lang.org t1,t2 --edit --title 'RUST'  # should prompt for unknown tags and overwrite title from web
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d add https://www.rust-lang.org t1,t2

.PHONY: run-search
run-search: init-db  ## run-search interactively for manual tests
	#BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo run -- -d -d search --np 1>/dev/null  # filter stderr out
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- -d -d search
	#pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo run -- search --json  # json output

.PHONY: init-db
init-db:  ## init-db
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo test --package bkmr --test test_lib test_dal::test_init_db -- --exact

.PHONY: install-diesel-cli
install-diesel-cli:  ## install-diesel-cli
	cargo install diesel_cli --no-default-features --features sqlite
	asdf reshim rust

.PHONY: test-vim
test-vim:  ## test-vim: run with EDITOR= make test-vim
	#pushd $(pkg_src) && cargo test --color=always --package bkmr --lib process::test::test_do_edit -- --nocapture --ignored
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db cargo test --color=always --test test_process test_do_edit -- --nocapture --ignored

.PHONY: test-dal
test-dal:  ## test-dal
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db RUST_LOG=DEBUG cargo test --package bkmr --test test_lib "" -- --test-threads=1

.PHONY: test
test:  test-dal  ## test (must run DB test before to init ?!?)
	#BKMR_DB_URL=../db/bkmr.db RUST_LOG=DEBUG pushd $(pkg_src) && cargo test --package bkmr -- --test-threads=1  # --nocapture
	pushd $(pkg_src) && BKMR_DB_URL=../db/bkmr.db RUST_LOG=DEBUG cargo test -- --test-threads=1  # --nocapture

.PHONY: test-with-data
test-with-data:  ## test-with-data
	pushd $(pkg_src) && BKMR_DB_URL=/Users/Q187392/dev/s/private/vimwiki/buku/bm.db_20230110_170737 cargo run -- search --fzf

.PHONY: benchmark
benchmark:  ## benchmark
	time BKMR_DB_URL=/Users/Q187392/dev/s/private/vimwiki/buku/bm.db_20230110_170737 /Users/Q187392/dev/s/private/rs-bkmr/bkmr/target/release/bkmr search zzzeek --np
	@echo "-----------------------------------------------------------"
	time bkmr_DB_URL=sqlite://///Users/Q187392/dev/s/private/vimwiki/buku/bm.db_20230110_170737 /Users/Q187392/.local/bin/bkmr search zzzeek --np


################################################################################
# Building, Deploying \
BUILDING:  ## ##################################################################

.PHONY: all
all: clean build install  ## all
	:

.PHONY: generate-ci
generate-ci:  ## generate-ci
	maturin generate-ci github --platform macos --platform linux -m bkmr/Cargo.toml

.PHONY: upload
upload:  ## upload
	pushd $(pkg_src) && cargo release publish --execute

.PHONY: build-wheel
build-wheel:  ## build-wheel
	maturin build --release -m bkmr/Cargo.toml

.PHONY: build
build:  ## build
	pushd $(pkg_src) && cargo build --release

.PHONY: install
install: uninstall  ## install
	@cp -vf bkmr/target/release/$(BINARY) ~/bin/$(BINARY)

.PHONY: uninstall
uninstall:  ## uninstall
	-@test -f ~/bin/$(BINARY) && rm -v ~/bin/$(BINARY)

.PHONY: bump-major
bump-major:  ## bump-major, tag and push
	bump-my-version bump --commit --tag major
	git push
	git push --tags
	@$(MAKE) create-release

.PHONY: bump-minor
bump-minor:  ## bump-minor, tag and push
	bump-my-version bump --commit --tag minor
	git push
	git push --tags
	@$(MAKE) create-release

.PHONY: bump-patch
bump-patch:  ## bump-patch, tag and push
	bump-my-version bump --commit --tag patch
	git push
	git push --tags
	@$(MAKE) create-release

.PHONY: create-release
create-release:  ## create a release on GitHub via the gh cli
	@if command -v gh version &>/dev/null; then \
		echo "Creating GitHub release for v$(VERSION)"; \
		gh release create "v$(VERSION)" --generate-notes; \
	else \
		echo "You do not have the github-cli installed. Please create release from the repo manually."; \
		exit 1; \
	fi

.PHONY: format
format:  ## format
	BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo fmt

.PHONY: lint
lint:  ## lint
	#BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo clippy
	BKMR_DB_URL=../db/bkmr.db pushd $(pkg_src) && cargo clippy --fix

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
