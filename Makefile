.PHONY: clean build run test test-verbose test-doc test-coverage lint lint-fix format fmt-check check help install install-python bench bench-exif prepare-bench-src

CARGO ?= cargo
PREFIX ?= ${HOME}/.local
BINDIR ?= ${PREFIX}/bin
RELEASE_BIN := target/release/pictures4096
VENV := .venv
PYTHON := ${VENV}/bin/python
PIP := ${VENV}/bin/pip
N ?= 50
PICTURES_DIR ?= ${HOME}/Desktop/Pictures
BENCH_SRC := /tmp/pictures4096-bench-src
BENCH_PY := /tmp/pictures4096-bench-py
BENCH_RS := /tmp/pictures4096-bench-rs
BENCH_PLAIN := /tmp/pictures4096-bench-plain
BENCH_EXIF_PY := /tmp/pictures4096-bench-exif-py
BENCH_EXIF_RS := /tmp/pictures4096-bench-exif-rs

help:
	@echo "Available targets:"
	@echo "  make install        - Build and install the pictures4096 binary"
	@echo "  make build          - Build the release binary"
	@echo "  make run            - Run pictures4096 (INPUT_DIR and OUTPUT_DIR required)"
	@echo "  make test           - Run Rust tests"
	@echo "  make lint           - Run Clippy with warnings denied"
	@echo "  make format         - Format Rust code"
	@echo "  make bench          - Compare Rust vs Python resize+EXIF on N photos (N=50)"
	@echo "  make bench-exif     - Compare exiftool vs fast-exif-rs EXIF writes"
	@echo "  make install-python - Install the legacy Python tool into .venv"
	@echo "  make clean          - Remove build artifacts and virtualenv"

${RELEASE_BIN}: Cargo.toml rustfmt.toml
	${CARGO} build --release

build: ${RELEASE_BIN}

check:
	${CARGO} check

install: ${RELEASE_BIN}
	mkdir -p ${BINDIR}
	install -m 0755 ${RELEASE_BIN} ${BINDIR}/pictures4096
	${CARGO} install --path . --force --locked

${VENV}:
	python3 -m venv ${VENV}
	${PIP} install --upgrade pip
	${PIP} install -e .

install-python: ${VENV}

run: ${RELEASE_BIN}
	@if [ -z "${INPUT_DIR}" ] || [ -z "${OUTPUT_DIR}" ]; then \
		echo "Usage: make run INPUT_DIR=/path/to/input OUTPUT_DIR=/path/to/output"; \
		exit 1; \
	fi
	${RELEASE_BIN} ${INPUT_DIR} ${OUTPUT_DIR}

test:
	${CARGO} test

test-verbose:
	${CARGO} test -- --nocapture

test-doc:
	${CARGO} test --doc

test-coverage:
	${CARGO} tarpaulin --out Html --output-dir coverage

lint: rustfmt.toml
	${CARGO} clippy --all-targets --all-features -- -D warnings

lint-fix:
	${CARGO} clippy --fix --allow-dirty --allow-staged

format: rustfmt.toml
	${CARGO} fmt

fmt-check: rustfmt.toml
	${CARGO} fmt --check

prepare-bench-src:
	@if [ ! -d "${PICTURES_DIR}" ]; then \
		echo "Pictures directory not found: ${PICTURES_DIR}"; \
		exit 1; \
	fi
	rm -rf ${BENCH_SRC}
	mkdir -p ${BENCH_SRC}
	@echo "Preparing ${N} hardlinked photos from ${PICTURES_DIR}"
	find "${PICTURES_DIR}" -type f \( \
		-iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.png' -o -iname '*.tif' -o -iname '*.tiff' -o -iname '*.webp' \
	\) | sort | head -n ${N} | while IFS= read -r src; do \
		rel=$${src#${PICTURES_DIR}/}; \
		dest="${BENCH_SRC}/$${rel}"; \
		mkdir -p "$$(dirname "$${dest}")"; \
		ln "$${src}" "$${dest}" 2>/dev/null || cp -l "$${src}" "$${dest}" 2>/dev/null || cp "$${src}" "$${dest}"; \
	done
	@count=$$(find ${BENCH_SRC} -type f | wc -l); \
	echo "Benchmark set: $${count} files"; \
	if [ "$${count}" -eq 0 ]; then echo "No images found to benchmark"; exit 1; fi

bench: ${RELEASE_BIN} ${VENV} prepare-bench-src
	@echo "=== Python resize + exiftool ==="
	rm -rf ${BENCH_PY}
	/usr/bin/time -f 'python wall seconds: %e' ${PYTHON} resize_images.py --no-resume --workers $$(nproc) "${BENCH_SRC}" "${BENCH_PY}"
	@echo "=== Rust resize + fast-exif-rs (default --copy-exif) ==="
	rm -rf ${BENCH_RS}
	/usr/bin/time -f 'rust wall seconds: %e' ${RELEASE_BIN} --no-resume --workers $$(nproc) "${BENCH_SRC}" "${BENCH_RS}"
	@echo "Python outputs: $$(find ${BENCH_PY} -type f | wc -l)"
	@echo "Rust outputs:   $$(find ${BENCH_RS} -type f | wc -l)"

bench-exif: ${RELEASE_BIN} ${VENV} prepare-bench-src
	@echo "=== Prepare resized files without EXIF ==="
	rm -rf ${BENCH_PLAIN} ${BENCH_EXIF_PY} ${BENCH_EXIF_RS}
	${RELEASE_BIN} --no-resume --no-copy-exif --quiet --workers $$(nproc) "${BENCH_SRC}" "${BENCH_PLAIN}"
	cp -a ${BENCH_PLAIN} ${BENCH_EXIF_PY}
	cp -a ${BENCH_PLAIN} ${BENCH_EXIF_RS}
	@echo "=== Python exiftool write ==="
	/usr/bin/time -f 'python exiftool wall seconds: %e' ${PYTHON} scripts/copy_exif_exiftool.py "${BENCH_SRC}" "${BENCH_EXIF_PY}"
	@echo "=== Rust fast-exif-rs write ==="
	/usr/bin/time -f 'rust fast-exif-rs wall seconds: %e' ${RELEASE_BIN} --exif-only --no-resume --workers $$(nproc) "${BENCH_SRC}" "${BENCH_EXIF_RS}"
	@echo "exiftool outputs:    $$(find ${BENCH_EXIF_PY} -type f | wc -l)"
	@echo "fast-exif-rs outputs: $$(find ${BENCH_EXIF_RS} -type f | wc -l)"

clean:
	${CARGO} clean
	rm -rf ${VENV}
	rm -rf __pycache__ .mypy_cache .ruff_cache coverage pictures4096.egg-info
	rm -rf ${BENCH_SRC} ${BENCH_PY} ${BENCH_RS} ${BENCH_PLAIN} ${BENCH_EXIF_PY} ${BENCH_EXIF_RS}
