BRAGI_VERSION = $(shell cat Cargo.toml | grep '^version' | cut -d '=' -f 2 | tr -d '[[:space:]]'\")

SHELL=/bin/bash

# Configuration
.PHONY: check help
.DEFAULT_GOAL := help

CLIPPY_PACKAGES := -p mimir -p common -p places
CLIPPY_EXTRA := --warn clippy::cargo --allow clippy::multiple_crate_versions --deny warnings

check: pre-build ## Runs several tests (alias for pre-build)
pre-build: fmt lint test

fmt: format ## Check formatting of the code (alias for 'format')
format: ## Check formatting of the code
	cargo fmt --all -- --check

clippy: lint ## Check quality of the code (alias for 'lint')
lint: ## Check quality of the code
	cargo clippy $(CLIPPY_PACKAGES) --all-targets -- $(CLIPPY_EXTRA)
	cargo clippy $(CLIPPY_PACKAGES) --bins --all-features -- $(CLIPPY_EXTRA)
	cargo clippy $(CLIPPY_PACKAGES) --all-targets --no-default-features -- $(CLIPPY_EXTRA)

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
