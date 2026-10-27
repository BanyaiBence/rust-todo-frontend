TODO_API_URL ?= http://localhost:3000
export TODO_API_URL

.DEFAULT_GOAL := help
.PHONY: help build release run run-release test test-live test-all clippy fmt fmt-check check ci clean

help: ## Show this help
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | awk -F ':.*## ' '{printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'

build: ## Build a debug binary
	cargo build

release: ## Build an optimized release binary
	cargo build --release

run: ## Run the app (debug) against TODO_API_URL
	cargo run

run-release: ## Run the app (release) against TODO_API_URL
	cargo run --release

test: ## Run tests that need no backend
	cargo test

test-live: ## Run only the live tests against TODO_API_URL
	cargo test -- --ignored

test-all: ## Run all tests, including live ones
	cargo test -- --include-ignored

clippy: ## Lint with clippy, warnings as errors
	cargo clippy --all-targets -- -D warnings

fmt: ## Format the code
	cargo fmt

fmt-check: ## Check formatting without changing files
	cargo fmt -- --check

check: fmt-check clippy test ## Run formatting check, clippy and tests

ci: check ## Alias for check

clean: ## Remove build artifacts
	cargo clean
