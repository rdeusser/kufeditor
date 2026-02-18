.PHONY: all build run test clean configure rebuild generate

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

# Generate C++ parsers from cleave (.clv) specs
generate:
	@for f in src/parsers/*.clv; do cleave generate --lang cpp --out src/parsers "$$f"; done

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
	@echo "  generate      - Regenerate C++ from .clv specs"
	@echo "  help          - Show this help message"
