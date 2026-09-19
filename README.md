# pictures4096

Batch image resizer that ingests a folder tree and writes copies whose longest edge fits a maximum size (default **4096** pixels). Aspect ratio is preserved. JPEG EXIF and other APPn segments are copied onto re-encoded JPEGs.

The supported implementation is a Rust `pictures4096` binary. The original Python script remains for comparison benchmarks.

## Features

- Fit-inside resize (no upscaling) with Lanczos3 via `fast_image_resize`
- Maximum CPU parallelism with Rayon (all logical cores by default)
- Multiple progress bars (overall throughput, successes, failures)
- Directory layout preserved
- Resume by skipping existing outputs
- `--limit` for sampling or benchmarks
- JPEG APPn metadata merge after encode

## Installation

### Prerequisites

- Rust 1.80+ (`cargo`, `rustc`)
- Optional: Python 3.10+ only if you want the legacy tool or `make bench`

```bash
make install
```

This builds the release binary and installs `pictures4096` to `${HOME}/.local/bin` and `~/.cargo/bin`.

```bash
pictures4096 --help
```

Legacy Python tool (used by `make bench`):

```bash
make install-python
```

## Usage

```bash
pictures4096 INPUT_DIR OUTPUT_DIR
```

```bash
make run INPUT_DIR=/path/to/photos OUTPUT_DIR=/path/to/photos_resized
```

### Options

```
pictures4096 [OPTIONS] <INPUT_DIR> <OUTPUT_DIR>

--max-size <PX>     Maximum edge length (default: 4096)
--quality <1-100>   JPEG quality (default: 95)
--workers <N>       Parallel workers (default: all logical CPUs)
--no-resume         Rewrite outputs that already exist
--quiet             Hide progress bars
--format <EXT>      Only this extension (for example .jpg)
--limit <N>         Process only the first N discovered files
```

### Examples

```bash
pictures4096 ./photos ./photos_resized
pictures4096 --max-size 2048 --quality 98 ./photos ./photos_resized
pictures4096 --format .jpg --workers 16 ./photos ./photos_resized
pictures4096 --limit 100 --no-resume ~/Desktop/Pictures /tmp/pictures4096-out
```

## Behavior

- Images larger than `--max-size` are scaled so the longer edge equals that size
- Smaller images are re-encoded without upscaling
- Output paths mirror the input tree
- Resume is on by default
- Failed files are reported and do not stop the batch

## Benchmark vs Python

Resize `N` photos from `~/Desktop/Pictures` into `/tmp` for both implementations:

```bash
make bench N=100
```

Override the source folder if needed:

```bash
make bench N=200 PICTURES_DIR=/path/to/photos
```

The benchmark hardlinks the first `N` sorted images into `/tmp/pictures4096-bench-src`, then writes:

- Python: `/tmp/pictures4096-bench-py`
- Rust: `/tmp/pictures4096-bench-rs`

## Development

```bash
make test
make lint
make format
make check
make clean
```

## License

MIT OR Apache-2.0
