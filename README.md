# pictures4096

Batch image resizer that ingests a folder tree and writes copies whose longest edge fits a maximum size (default **4096** pixels). Aspect ratio is preserved. EXIF is copied by default with [fast-exif-rs](https://github.com/dapperfu/fast-exif-rs) (`--copy-exif`).

The supported implementation is a Rust `pictures4096` binary. The original Python script remains for comparison benchmarks.

## Features

- Fit-inside resize (no upscaling) with Lanczos3 via `fast_image_resize`
- Maximum CPU parallelism with Rayon (all logical cores by default)
- Multiple progress bars (overall throughput, successes, failures)
- Directory layout preserved
- Resume by skipping existing outputs
- `--limit` for sampling or benchmarks
- Default `--copy-exif` using [fast-exif-rs](https://github.com/dapperfu/fast-exif-rs) (high-priority subset plus JPEG `APPn` fallback)
- `--no-copy-exif` and `--exif-only` for isolated metadata writes

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
--copy-exif         Copy EXIF with fast-exif-rs (default: on)
--no-copy-exif      Skip EXIF copy
--exif-only         Write EXIF onto existing outputs (no resize)
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

- Python resize + exiftool: `/tmp/pictures4096-bench-py`
- Rust resize + fast-exif-rs: `/tmp/pictures4096-bench-rs`

Isolated EXIF write (same resized files, then metadata only):

```bash
make bench-exif N=100
```

- Python `exiftool -tagsFromFile`: `/tmp/pictures4096-bench-exif-py`
- Rust `--exif-only` via fast-exif-rs: `/tmp/pictures4096-bench-exif-rs`

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
