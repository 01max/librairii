SHELL := /bin/sh

# Rust-only application commands. Version pins live in rust-toolchain.toml and
# .config/rust-tools.toml. GUI execution remains an explicit developer action.
.PHONY: setup fmt check check-rust test build run package

setup:
	cargo xtask tools

fmt:
	cargo fmt --all

check:
	cargo xtask check

check-rust: check

test:
	cargo nextest run --locked --workspace --profile ci

build:
	cargo build --locked --release -p librairii-desktop

run:
	cargo run --locked -p librairii-desktop

# Preview only. Pass TARGET and NAME; add --execute to the printed helper
# invocation yourself when you want a package built on its matching host.
package:
	python3 -I -B scripts/build-native-package.py --target "$(TARGET)" --name "$(NAME)"
