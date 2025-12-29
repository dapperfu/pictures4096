# Batch Image Resizer with EXIF Preservation

A Python CLI tool for batch resizing images to a maximum dimension (default 4096x4096) while preserving aspect ratio, copying EXIF metadata, and maintaining directory structure. Designed for archival purposes with maximum parallelization.

## Features

- **Aspect Ratio Preservation**: Images are resized to fit within the maximum dimension while maintaining proportions
- **EXIF Metadata Preservation**: Copies all EXIF data from source images to resized images
- **Directory Structure Preservation**: Maintains the same directory structure in the output location
- **Parallel Processing**: Automatically detects CPU cores and processes images in parallel
- **Resume Support**: Skips already processed images to allow resuming interrupted batches
- **Progress Reporting**: Rich dashboard with file-by-file progress, statistics, and ETA
- **Image Validation**: Verifies that saved images are valid and readable
- **Comprehensive Error Handling**: Continues processing on errors with detailed logging

## Installation

### Prerequisites

- Python 3.10 or higher
- pip or uv package manager
- exiftool command-line tool (required for EXIF metadata handling)

### Installing exiftool

**Linux (Ubuntu/Debian):**
```bash
sudo apt install libimage-exiftool-perl
```

**Linux (CentOS/RHEL):**
```bash
sudo yum install perl-Image-ExifTool
```

**macOS:**
```bash
brew install exiftool
```

**Windows:**
Download from https://exiftool.org/ and add to PATH

Verify installation:
```bash
exiftool -ver
```

### Setup

1. Clone or download this repository

2. Install dependencies using the Makefile:
```bash
make install
```

Or manually:
```bash
python3 -m venv .venv
source .venv/bin/activate  # On Windows: .venv\Scripts\activate
pip install -e .
```

## Usage

### Basic Usage

```bash
python resize_images.py INPUT_DIR OUTPUT_DIR
```

### With Makefile

```bash
make run INPUT_DIR=/path/to/photos OUTPUT_DIR=/path/to/photos_resized
```

### Command-Line Options

```bash
resize_images.py [OPTIONS] INPUT_DIR OUTPUT_DIR

Options:
  --max-size INTEGER          Maximum dimension (width or height) in pixels (default: 4096)
  --quality INTEGER           JPEG quality 1-100 (default: 95)
  --workers INTEGER           Number of parallel workers (default: auto-detect CPU cores)
  --no-resume                 Reprocess all images even if output exists
  --quiet                     Minimal output
  --format TEXT               Only process specific format (e.g., '.jpg')
  --help                      Show this message and exit
```

### Examples

Resize all images to 4096x4096 (default):
```bash
python resize_images.py ./photos ./photos_resized
```

Resize to 2048x2048 with high quality:
```bash
python resize_images.py --max-size 2048 --quality 98 ./photos ./photos_resized
```

Process only JPEG files:
```bash
python resize_images.py --format .jpg ./photos ./photos_resized
```

Reprocess all images (ignore existing outputs):
```bash
python resize_images.py --no-resume ./photos ./photos_resized
```

Use specific number of workers:
```bash
python resize_images.py --workers 8 ./photos ./photos_resized
```

## Behavior

### Image Resizing

- Images larger than the maximum dimension are resized to fit within it while preserving aspect ratio
- Images smaller than or equal to the maximum dimension are copied as-is (no upscaling)
- Uses high-quality LANCZOS resampling algorithm

### EXIF Metadata

- Attempts to copy all EXIF metadata from source to output
- If EXIF copying fails, the image is still saved but without EXIF data
- Detailed error messages are logged for troubleshooting

### Directory Structure

The output directory maintains the same structure as the input directory:

```
photos/
  vacation/
    beach.jpg
    sunset.png
  family/
    photo1.jpg

photos_resized/
  vacation/
    beach.jpg
    sunset.png
  family/
    photo1.jpg
```

### Resume Capability

By default, the script skips images that have already been processed (output file exists). Use `--no-resume` to reprocess all images.

## Supported Formats

All image formats supported by Pillow, including:
- JPEG (.jpg, .jpeg)
- PNG (.png)
- TIFF (.tiff, .tif)
- GIF (.gif)
- BMP (.bmp)
- WebP (.webp)
- And many more...

## Development

### Linting

```bash
make lint
```

### Formatting

```bash
make format
```

### Clean

```bash
make clean
```

## Requirements

- Python 3.10+
- click >= 8.1.0
- Pillow >= 10.0.0
- PyExifTool >= 0.5.6
- rich >= 13.0.0
- exiftool >= 12.15 (system dependency - must be installed separately)

## License

This project is provided as-is for archival purposes.

## Troubleshooting

### EXIF Errors

If you see EXIF-related warnings, the images are still processed but without metadata. This is normal for some image formats or corrupted EXIF data.

### Memory Issues

For very large batches, consider:
- Processing in smaller batches
- Reducing the number of workers with `--workers`
- Using a machine with more RAM

### Permission Errors

Ensure you have read access to the input directory and write access to the output directory.

