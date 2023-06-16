BRAGI_VERSION = $(shell cat Cargo.toml | grep '^version' | cut -d '=' -f 2 | tr -d '[[:space:]]'\")

SHELL=/bin/bash

# Configuration
.PHONY: check help
.DEFAULT_GOAL := help

CLIPPY_EXTRA := --allow clippy::multiple_crate_versions --deny warnings

check: pre-build ## Runs several tests (alias for pre-build)
pre-build: fmt lint test

fmt: format ## Check formatting of the code (alias for 'format')
format: ## Check formatting of the code
	cargo fmt --all -- --check

clippy: lint ## Check quality of the code (alias for 'lint')
lint: ## Check quality of the code
	cargo clippy --all -- --allow clippy::multiple_crate_versions --deny warnings

test: ## Launch all tests
	cargo test --lib
	cargo test --bins
	cargo test --doc
	cargo test --test end_to_end
	cargo test --package mimir
	cargo test --package common

.PHONY: version
version: ## display version of bragi
	@echo $(BRAGI_VERSION)
