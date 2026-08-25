.PHONY: all build run test clean configure rebuild generate check-generated tidy rust-build rust-test rust-run rust-lint rust-deny rust-release rust-check

BUILD_DIR := build
BUILD_TYPE := Release

all: build

configure:
	@mkdir -p $(BUILD_DIR)
	@cd $(BUILD_DIR) && cmake .. -G Ninja -DCMAKE_BUILD_TYPE=$(BUILD_TYPE)

build: configure
	@cmake --build $(BUILD_DIR) --config $(BUILD_TYPE) --parallel

run: build
	@./$(BUILD_DIR)/kufeditor

test: build
	@cd $(BUILD_DIR) && ctest -C $(BUILD_TYPE) --output-on-failure

clean:
	@rm -rf $(BUILD_DIR)

rebuild: clean build

# Generate C++ and Rust parsers from the shared cleave schemas.
generate:
	@./scripts/generate.sh

check-generated:
	@./scripts/check-generated.sh

rust-build:
	@cargo build --workspace

rust-test:
	@cargo test --workspace --all-features

rust-run:
	@cargo run -p kufeditor

rust-lint:
	@cargo fmt --all --check
	@cargo clippy --workspace --all-targets --all-features -- -D warnings

rust-deny:
	@cargo deny check

rust-release:
	@cargo build --release -p kufeditor

rust-check: rust-lint rust-test rust-deny rust-release check-generated

# Debug build variants
debug:
	@$(MAKE) BUILD_TYPE=Debug build

run-debug:
	@$(MAKE) BUILD_TYPE=Debug build
	@./$(BUILD_DIR)/kufeditor

# Verbose build
build-verbose: configure
	@cmake --build $(BUILD_DIR) --config $(BUILD_TYPE) --parallel --verbose

# Just run tests without rebuilding
test-only:
	@cd $(BUILD_DIR) && ctest -C $(BUILD_TYPE) --output-on-failure

tidy:
	@fd -e cpp -e h . src test | xargs clang-format -i

help:
	@echo "Available targets:"
	@echo "  build         - Build the project (default)"
	@echo "  run           - Build and run the application"
	@echo "  test          - Build and run tests"
	@echo "  clean         - Remove build directory"
	@echo "  rebuild       - Clean and build"
	@echo "  debug         - Build in debug mode"
	@echo "  run-debug     - Build and run in debug mode"
	@echo "  configure     - Run CMake configuration"
	@echo "  build-verbose - Build with verbose output"
	@echo "  test-only     - Run tests without rebuilding"
	@echo "  generate      - Regenerate C++ and Rust parsers"
	@echo "  check-generated - Verify generated parsers are current"
	@echo "  rust-build    - Build the Rust workspace"
	@echo "  rust-test     - Run all Rust tests"
	@echo "  rust-run      - Run the GPUI application"
	@echo "  rust-lint     - Check Rust formatting and lints"
	@echo "  rust-deny     - Audit Rust dependencies"
	@echo "  rust-release  - Build the release GPUI application"
	@echo "  rust-check    - Run every local Rust and generation gate"
	@echo "  tidy          - Format all .cpp and .h files with clang-format"
	@echo "  help          - Show this help message"
