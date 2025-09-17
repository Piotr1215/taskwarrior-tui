.PHONY: help build test test-repo install clean check release run fmt clippy all

# Default target
all: fmt check test build

help: ## Show this help message
	@echo 'Usage: make [target]'
	@echo ''
	@echo 'Available targets:'
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "}; {printf "  %-15s %s\n", $$1, $$2}'

build: ## Build the project in debug mode
	cargo build

release: ## Build the project in release mode
	cargo build --release

test: ## Run all tests
	cargo test

test-repo: ## Run repo-specific tests
	cargo test test_repo

test-verbose: ## Run tests with output
	cargo test -- --nocapture

check: ## Check if the project compiles
	cargo check

fmt: ## Format the code
	cargo fmt

clippy: ## Run clippy lints
	cargo clippy -- -D warnings

install: release ## Install the binary locally
	cargo install --path . --locked

clean: ## Clean build artifacts
	cargo clean

run: ## Run the application
	cargo run

watch: ## Watch for changes and rebuild
	cargo watch -x check -x test -x run