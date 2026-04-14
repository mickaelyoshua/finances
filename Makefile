# Finances TUI — development workflow
# Usage: make <target>

.PHONY: dev
dev: ## Start local PostgreSQL via docker-compose
	docker compose up -d

.PHONY: migrate
migrate: ## Run migrations on dev database
	cargo run -- --migrate

.PHONY: seed
seed: ## Reset dev DB to demo data from seeds.sql
	docker exec -i finances-tui-db-1 psql -U finances -d finances < seeds.sql

.PHONY: migrate-prod
migrate-prod: ## Run migrations on production database (Neon)
	cargo run -- --migrate --prod

.PHONY: run
run: ## Launch TUI on dev database
	cargo run

.PHONY: run-prod
run-prod: ## Launch TUI on production database
	cargo run -- --prod

.PHONY: test
test: ## Run all tests (migrates test DB internally)
	cargo test

.PHONY: test-one
test-one: ## Run a single test: make test-one T=test_name
	cargo test $(T)

.PHONY: build
build: ## Debug build
	cargo build

.PHONY: release
release: ## Run tests then optimized release build
	cargo test
	cargo build --release

.PHONY: clippy
clippy: ## Run clippy lints
	cargo clippy

.PHONY: deploy
deploy: ## Full pipeline: test → migrate dev → migrate prod → release build
	cargo test
	cargo run -- --migrate
	cargo run -- --migrate --prod
	cargo build --release

.PHONY: help
help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "  \033[36m%-14s\033[0m %s\n", $$1, $$2}'
