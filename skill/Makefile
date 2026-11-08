.DEFAULT_GOAL := help
MAKEFLAGS += --no-print-directory

app_root := $(CURDIR)
hooks := $(app_root)/hooks
scripts := $(app_root)/bkmr-memory/scripts
evals := $(app_root)/bkmr-memory/evals
python_src := $(hooks) $(scripts) $(evals)

# The scripts are stdlib-only PEP 723 scripts. Pytest and ruff come in ad hoc via `uv run --with`.
# --no-project: this directory sits inside the bkmr Python project, whose build runs maturin.
UV := uv run --quiet --no-project
PYTEST := $(UV) --with pytest pytest

################################################################################
# Testing \
TESTING:  ## ############################################################

.PHONY: test
test:  ## run all pytest cases (hooks, due report, audit runner)
	$(PYTEST) $(hooks) $(scripts)

.PHONY: test-verbose
test-verbose:  ## run pytest with -v
	$(PYTEST) -v $(hooks) $(scripts)

################################################################################
# Code Quality \
QUALITY:  ## ############################################################

.PHONY: lint
lint:  ## ruff check on all Python scripts and tests
	@$(UV) --with ruff ruff check $(python_src)

.PHONY: format
format:  ## ruff format on all Python scripts and tests
	@$(UV) --with ruff ruff format $(python_src)

################################################################################
# Clean \
CLEAN:  ## ############################################################

.PHONY: clean
clean: clean-pyc clean-pytest  ## remove Python and pytest artifacts

.PHONY: clean-pyc
clean-pyc:  ## remove Python file artifacts
	find . -name '*.pyc' -exec rm -f {} +
	find . -name '*.pyo' -exec rm -f {} +
	find . -name '__pycache__' -exec rm -fr {} +

.PHONY: clean-pytest
clean-pytest:  ## remove pytest cache
	find . -name '.pytest_cache' -exec rm -fr {} +

################################################################################
# Misc \
MISC:  ## ############################################################

define PRINT_HELP_PYSCRIPT
import re, sys

for line in sys.stdin:
	match = re.match(r'^([a-zA-Z0-9_-]+):.*?## (.*)$$', line)
	if match:
		target, help = match.groups()
		print("\033[36m%-20s\033[0m %s" % (target, help))
endef
export PRINT_HELP_PYSCRIPT

.PHONY: help
help:  ## print this help
	@$(UV) python -c "$$PRINT_HELP_PYSCRIPT" < $(MAKEFILE_LIST)
