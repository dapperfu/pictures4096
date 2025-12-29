.PHONY: clean build run test lint format help install

# Python virtual environment
VENV = .venv
PYTHON = $(VENV)/bin/python
PIP = $(VENV)/bin/pip

# Default target
help:
	@echo "Available targets:"
	@echo "  make install  - Install dependencies in virtual environment"
	@echo "  make build    - Build the project (no-op for Python project)"
	@echo "  make run       - Run the resize_images script (requires INPUT_DIR and OUTPUT_DIR)"
	@echo "  make test      - Run tests (if available)"
	@echo "  make lint      - Run linting checks (ruff and mypy)"
	@echo "  make format    - Format code with ruff"
	@echo "  make clean     - Remove build artifacts and virtual environment"

# Install dependencies
install: $(VENV)
	$(PIP) install -e .

# Create virtual environment if it doesn't exist
$(VENV):
	python3 -m venv $(VENV)
	$(PIP) install --upgrade pip
	$(PIP) install -e .

# Build (no-op for Python project, but ensure venv exists)
build: $(VENV)
	@echo "Build complete (Python project - no compilation needed)"

# Run the script
run: $(VENV)
	@if [ -z "$(INPUT_DIR)" ] || [ -z "$(OUTPUT_DIR)" ]; then \
		echo "Usage: make run INPUT_DIR=/path/to/input OUTPUT_DIR=/path/to/output"; \
		echo "Example: make run INPUT_DIR=./photos OUTPUT_DIR=./photos_resized"; \
		exit 1; \
	fi
	$(PYTHON) resize_images.py $(INPUT_DIR) $(OUTPUT_DIR)

# Run tests (placeholder - add tests if needed)
test: $(VENV)
	@echo "No tests defined yet"

# Lint code
lint: $(VENV)
	$(VENV)/bin/ruff check resize_images.py
	$(VENV)/bin/mypy resize_images.py

# Format code
format: $(VENV)
	$(VENV)/bin/ruff format resize_images.py

# Clean build artifacts
clean:
	rm -rf $(VENV)
	rm -rf __pycache__
	rm -rf *.pyc
	rm -rf .mypy_cache
	rm -rf .ruff_cache
	find . -type d -name __pycache__ -exec rm -rf {} + 2>/dev/null || true
	find . -type f -name "*.pyc" -delete 2>/dev/null || true

