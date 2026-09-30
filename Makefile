# One target directory even when the shell has a global Cargo override.
CARGO ?= cargo
export CARGO_TARGET_DIR := target

.PHONY: help fmt fmt-check lint test build clean deny audit-unsafe docs spec coverage ci
help:
	@echo "Targets: fmt fmt-check lint test build clean deny audit-unsafe docs spec coverage ci"
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
	$(CARGO) deny --all-features check
audit-unsafe:
	bash scripts/check_unsafe_comments.sh
docs:
	RUSTDOCFLAGS="-D warnings" $(CARGO) doc --workspace --no-deps --all-features
spec:
	quire validate --scope . "spec/**/*.md"
	quire validate --scope . "reviews/**/*.md"
coverage:
	quire coverage --scope .
ci: fmt-check lint test deny audit-unsafe docs spec
