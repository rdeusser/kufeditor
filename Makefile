.PHONY: all build run test clean rebuild generate check-generated fmt fmt-check lint acronyms deny release check help

all: build

build:
	@cargo build --workspace --locked

run:
	@cargo run --locked -p kufeditor

test:
	@cargo test --workspace --all-targets --all-features --locked

clean:
	@cargo clean

rebuild: clean build

generate:
	@./scripts/generate.sh

check-generated:
	@./scripts/check-generated.sh

fmt:
	@cargo fmt --all

fmt-check:
	@cargo fmt --all -- --check

lint:
	@cargo clippy --workspace --all-targets --all-features --locked -- -D warnings

acronyms:
	@./scripts/check-rust-acronyms.sh

deny:
	@cargo deny check

release:
	@cargo build --release --locked -p kufeditor

check: fmt-check acronyms lint test deny release check-generated

help:
	@echo "Available targets:"
	@echo "  build           - Build the Rust workspace (default)"
	@echo "  run             - Build and run the GPUI application"
	@echo "  test            - Run all Rust tests"
	@echo "  clean           - Remove Cargo build output"
	@echo "  rebuild         - Clean and build the Rust workspace"
	@echo "  generate        - Regenerate Cleave Rust parsers"
	@echo "  check-generated - Verify generated Rust parsers are current"
	@echo "  fmt             - Format the Rust workspace"
	@echo "  fmt-check       - Verify Rust formatting"
	@echo "  lint            - Run strict Rust lints"
	@echo "  acronyms        - Verify project-owned acronym spelling"
	@echo "  deny            - Audit Rust dependencies"
	@echo "  release         - Build the optimized GPUI application"
	@echo "  check           - Run every local release gate"
	@echo "  help            - Show this help message"
