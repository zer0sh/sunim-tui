# Common tasks for sunim. Run `make` (or `make help`) to list them.

# Check PATH first; fallback to ~/.cargo/bin/cargo if present
CARGO_DETECTED := $(shell which cargo 2>/dev/null || { [ -x "$$HOME/.cargo/bin/cargo" ] && echo "$$HOME/.cargo/bin/cargo"; } || echo "cargo")
CARGO          ?= $(CARGO_DETECTED)
BIN            := sunim
VERSION        := $(shell sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)
MUSL           := x86_64-unknown-linux-musl

.DEFAULT_GOAL := help
.PHONY: help check-cargo build release static macos macos-universal package dist run test lint fmt fmt-check check ci install uninstall clean version tag

help: ## List available targets
	@grep -E '^[a-zA-Z_-]+:.*?## ' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*?## "} {printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'

check-cargo: ## Verify that Rust and Cargo (1.98+) are installed
	@which $(CARGO) >/dev/null 2>&1 || { \
		echo ""; \
		echo " \033[31m[ERROR]\033[0m Cargo was not found in PATH or in \$$HOME/.cargo/bin/cargo."; \
		echo " Building sunim explicitly requires Rust 1.98+ and Cargo (Rust edition 2024)."; \
		echo ""; \
		echo " To install Rust and Cargo on Linux or macOS, run:"; \
		echo "   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"; \
		echo ""; \
		echo " Then add Cargo to your current terminal environment:"; \
		echo "   source \"\$$HOME/.cargo/env\""; \
		echo ""; \
		exit 1; \
	}

build: check-cargo ## Debug build
	$(CARGO) build

release: check-cargo ## Optimized, size-tuned build (target/release/sunim)
	$(CARGO) build --release --locked

static: check-cargo ## Fully static Linux binary (target/x86_64-unknown-linux-musl/release/sunim)
	rustup target add $(MUSL)
	$(CARGO) build --release --locked --target $(MUSL)

macos: check-cargo ## Optimized release build for current macOS architecture
	$(CARGO) build --release --locked

macos-universal: check-cargo ## Universal macOS binary (x86_64 + Apple Silicon aarch64)
	rustup target add x86_64-apple-darwin aarch64-apple-darwin
	$(CARGO) build --release --locked --target x86_64-apple-darwin
	$(CARGO) build --release --locked --target aarch64-apple-darwin
	@mkdir -p target/universal/release
	lipo -create -output target/universal/release/sunim target/x86_64-apple-darwin/release/sunim target/aarch64-apple-darwin/release/sunim
	@echo "Universal binary created: target/universal/release/sunim"

package: check-cargo ## Build Cargo crate package (target/package/sunim-tui-<version>.crate)
	$(CARGO) package --allow-dirty
	@echo "Package ready: target/package/sunim-tui-$(VERSION).crate"

dist: release ## Create release tarball archive with prebuilt binary, README and LICENSE
	@mkdir -p dist/sunim-v$(VERSION)
	@cp target/release/sunim dist/sunim-v$(VERSION)/
	@cp README.md LICENSE dist/sunim-v$(VERSION)/
	@tar -czf dist/sunim-v$(VERSION)-$$(uname -s | tr '[:upper:]' '[:lower:]')-$$(uname -m).tar.gz -C dist sunim-v$(VERSION)
	@rm -rf dist/sunim-v$(VERSION)
	@echo "Distribution archive created in dist/"

run: check-cargo ## Build and launch the TUI
	$(CARGO) run

test: check-cargo ## Run the unit tests
	$(CARGO) test

lint: check-cargo ## Clippy on all targets, warnings are errors
	$(CARGO) clippy --all-targets -- -D warnings

fmt: check-cargo ## Format the code
	$(CARGO) fmt

fmt-check: check-cargo ## Fail if the code is not formatted
	$(CARGO) fmt --check

check: fmt-check lint test ## Everything CI runs: format, lint, tests

ci: check ## Alias of check

install: check-cargo ## Install the `sunim` binary into ~/.cargo/bin
	$(CARGO) install --path . --locked

uninstall: check-cargo ## Remove the installed binary
	$(CARGO) uninstall sunim-tui

clean: check-cargo ## Remove build artifacts
	$(CARGO) clean

version: ## Print the version from Cargo.toml
	@echo $(VERSION)

tag: check ## Tag vX.Y.Z from Cargo.toml (push the tag to publish a release)
	@git diff --quiet && git diff --cached --quiet || { echo "Commit your changes before tagging."; exit 1; }
	@! git rev-parse -q --verify "refs/tags/v$(VERSION)" >/dev/null || { echo "Tag v$(VERSION) already exists: bump version in Cargo.toml."; exit 1; }
	git tag -a "v$(VERSION)" -m "$(BIN) v$(VERSION)"
	@echo "Publish it with: git push origin v$(VERSION)"
