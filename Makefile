# One target directory even when the shell has a global Cargo override.
CARGO ?= cargo
export CARGO_TARGET_DIR := target

.PHONY: help fmt fmt-check lint test build clean deny audit-unsafe docs docs-check docs-examples spec coverage ci
help:
	@echo "Targets: fmt fmt-check lint test build clean deny audit-unsafe docs docs-check docs-examples spec coverage ci"
fmt:
	$(CARGO) fmt --all
fmt-check:
	$(CARGO) fmt --all -- --check
lint:
	$(CARGO) clippy --workspace --all-targets --all-features -- -D warnings
	$(CARGO) clippy --workspace --all-targets --no-default-features -- -D warnings
test:
	$(CARGO) test --workspace --all-features
	$(CARGO) test --workspace --no-default-features
build:
	$(CARGO) build --workspace --release --all-features
clean:
	$(CARGO) clean
deny:
	$(CARGO) deny --workspace --all-features --locked check
audit-unsafe:
	bash scripts/check_unsafe_comments.sh
docs:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps --all-features
docs-examples:
	$(CARGO) check --locked --workspace --examples --all-features
	$(CARGO) run --locked --example reference
	$(CARGO) run --locked -p sapho-select --example acquisition
	$(CARGO) run --locked -p sapho-evidence --example measurement
	$(CARGO) run --locked -p sapho-clm --example clm_configure
	$(CARGO) build --locked -p sapho-cli --no-default-features
	python3 scripts/check_docs.py
docs-check: docs
	$(CARGO) test --locked --workspace --doc --all-features
	$(CARGO) test --locked --workspace --doc --no-default-features
	$(MAKE) docs-examples
spec:
	quire validate --scope . "spec/**/*.md"
	quire validate --scope . "reviews/**/*.md"
coverage:
	quire coverage --scope .
ci: fmt-check lint test deny audit-unsafe docs docs-examples spec
