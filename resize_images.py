#!/usr/bin/env python3
"""Batch image resizer with EXIF preservation for archival purposes."""

import logging
import os
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import List, Optional, Tuple

import click
from PIL import Image
from rich.console import Console
from rich.logging import RichHandler
from rich.progress import Progress, SpinnerColumn, BarColumn, TextColumn, TimeElapsedColumn

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(message)s",
    datefmt="[%X]",
    handlers=[RichHandler(rich_tracebacks=True)],
)
logger = logging.getLogger(__name__)

console = Console()

# Supported image formats by Pillow
SUPPORTED_FORMATS = {
    ".jpg", ".jpeg", ".png", ".gif", ".bmp", ".tiff", ".tif",
    ".webp", ".ico", ".pcx", ".ppm", ".pgm", ".pbm", ".xbm",
    ".xpm", ".svg", ".eps", ".psd", ".dib", ".jfif", ".jp2",
    ".jpx", ".j2k", ".j2c", ".fpx", ".fits", ".h5", ".hdf",
}


def discover_images(input_dir: Path, format_filter: Optional[str] = None) -> List[Path]:
    """
    Recursively discover all image files in the input directory.

    Parameters
    ----------
    input_dir : Path
        Root directory to search for images
    format_filter : Optional[str]
        Optional format filter (e.g., '.jpg') to limit file types

    Returns
    -------
    List[Path]
        List of image file paths found
    """
    images: List[Path] = []
    format_lower = format_filter.lower() if format_filter else None

    for root, dirs, files in os.walk(input_dir):
        for file in files:
            file_path = Path(root) / file
            suffix = file_path.suffix.lower()

            if format_lower:
                if suffix == format_lower:
                    images.append(file_path)
            elif suffix in SUPPORTED_FORMATS:
                images.append(file_path)

    return sorted(images)


def get_image_info(image_path: Path) -> Tuple[int, int, str]:
    """
    Get image dimensions and format.

    Parameters
    ----------
    image_path : Path
        Path to the image file

    Returns
    -------
    Tuple[int, int, str]
        Width, height, and format of the image
    """
    try:
        with Image.open(image_path) as img:
            return img.width, img.height, img.format or "UNKNOWN"
    except Exception as e:
        logger.error(f"Failed to get info for {image_path}: {e}")
        raise


def resize_image(
    source_path: Path,
    output_path: Path,
    max_size: int,
    quality: int,
) -> bool:
    """
    Resize image preserving aspect ratio if larger than max_size.

    Parameters
    ----------
    source_path : Path
        Path to source image
    output_path : Path
        Path to save resized image
    max_size : int
        Maximum dimension (width or height)
    quality : int
        JPEG quality (1-100)

    Returns
    -------
    bool
        True if successful, False otherwise
    """
    try:
        with Image.open(source_path) as img:
            width, height = img.size

            # If image is smaller than max_size, just copy it
            if width <= max_size and height <= max_size:
                img.save(output_path, quality=quality, exif=img.info.get("exif"))
                return True

            # Resize preserving aspect ratio
            img.thumbnail((max_size, max_size), Image.Resampling.LANCZOS)
            img.save(output_path, quality=quality, exif=img.info.get("exif"))
            return True

    except Exception as e:
        logger.error(f"Failed to resize {source_path}: {e}")
        return False


def copy_exif(source_path: Path, output_path: Path) -> bool:
    """
    Copy EXIF metadata from source to output image.

    Parameters
    ----------
    source_path : Path
        Path to source image with EXIF
    output_path : Path
        Path to output image to receive EXIF

    Returns
    -------
    bool
        True if EXIF copied successfully, False otherwise
    """
    try:
        import piexif

        exif_dict = piexif.load(str(source_path))
        exif_bytes = piexif.dump(exif_dict)
        piexif.insert(exif_bytes, str(output_path))
        return True
    except Exception as piexif_error:
        logger.warning(
            f"Failed to copy EXIF from {source_path} to {output_path}: {piexif_error}",
            exc_info=True,
        )
        return False


def validate_image(image_path: Path) -> bool:
    """
    Validate that the saved image is readable and valid.

    Parameters
    ----------
    image_path : Path
        Path to image to validate

    Returns
    -------
    bool
        True if image is valid, False otherwise
    """
    try:
        with Image.open(image_path) as img:
            img.verify()
        return True
    except Exception as e:
        logger.error(f"Validation failed for {image_path}: {e}")
        return False


def process_image(
    source_path: Path,
    input_dir: Path,
    output_dir: Path,
    max_size: int,
    quality: int,
    resume: bool,
) -> Tuple[bool, str]:
    """
    Process a single image: resize, copy EXIF, validate.

    Parameters
    ----------
    source_path : Path
        Path to source image
    input_dir : Path
        Root input directory
    output_dir : Path
        Root output directory
    max_size : int
        Maximum dimension
    quality : int
        JPEG quality
    resume : bool
        Whether to skip if output already exists

    Returns
    -------
    Tuple[bool, str]
        (success, status_message)
    """
    # Calculate relative path and output path
    try:
        rel_path = source_path.relative_to(input_dir)
        output_path = output_dir / rel_path

        # Check if already processed
        if resume and output_path.exists():
            return True, "skipped (already exists)"

        # Create output directory
        output_path.parent.mkdir(parents=True, exist_ok=True)

        # Resize or copy image
        if not resize_image(source_path, output_path, max_size, quality):
            return False, "resize failed"

        # Copy EXIF (log but continue on failure)
        copy_exif(source_path, output_path)

        # Validate saved image
        if not validate_image(output_path):
            return False, "validation failed"

        return True, "success"

    except Exception as e:
        logger.error(f"Error processing {source_path}: {e}", exc_info=True)
        return False, f"error: {str(e)}"


@click.command()
@click.argument("input_dir", type=click.Path(exists=True, file_okay=False, path_type=Path))
@click.argument("output_dir", type=click.Path(file_okay=False, path_type=Path))
@click.option(
    "--max-size",
    type=int,
    default=4096,
    help="Maximum dimension (width or height) in pixels (default: 4096)",
)
@click.option(
    "--quality",
    type=int,
    default=95,
    help="JPEG quality 1-100 (default: 95)",
)
@click.option(
    "--workers",
    type=int,
    default=None,
    help="Number of parallel workers (default: auto-detect CPU cores)",
)
@click.option(
    "--no-resume",
    is_flag=True,
    default=False,
    help="Reprocess all images even if output exists",
)
@click.option(
    "--quiet",
    is_flag=True,
    default=False,
    help="Minimal output",
)
@click.option(
    "--format",
    "format_filter",
    type=str,
    default=None,
    help="Only process specific format (e.g., '.jpg')",
)
def main(
    input_dir: Path,
    output_dir: Path,
    max_size: int,
    quality: int,
    workers: Optional[int],
    no_resume: bool,
    quiet: bool,
    format_filter: Optional[str],
) -> None:
    """
    Batch resize images with EXIF preservation.

    Processes all images in INPUT_DIR, resizes them to a maximum dimension
    (preserving aspect ratio), copies EXIF metadata, and saves them to
    OUTPUT_DIR maintaining the same directory structure.
    """
    if not quiet:
        console.print(f"[bold green]Batch Image Resizer[/bold green]")
        console.print(f"Input: {input_dir}")
        console.print(f"Output: {output_dir}")
        console.print(f"Max size: {max_size}px")
        console.print(f"Quality: {quality}")

    # Discover images
    if not quiet:
        console.print("\n[yellow]Discovering images...[/yellow]")
    images = discover_images(input_dir, format_filter)
    if not quiet:
        console.print(f"Found {len(images)} image(s)")

    if not images:
        console.print("[red]No images found![/red]")
        sys.exit(1)

    # Filter out already processed if resume enabled
    resume = not no_resume
    if resume:
        images_to_process = [
            img
            for img in images
            if not (output_dir / img.relative_to(input_dir)).exists()
        ]
        skipped = len(images) - len(images_to_process)
        if not quiet and skipped > 0:
            console.print(f"Skipping {skipped} already processed image(s)")
        images = images_to_process

    if not images:
        console.print("[green]All images already processed![/green]")
        sys.exit(0)

    # Determine worker count
    if workers is None:
        workers = os.cpu_count() or 1

    # Process images in parallel
    success_count = 0
    fail_count = 0

    with Progress(
        SpinnerColumn(),
        TextColumn("[progress.description]{task.description}"),
        BarColumn(),
        TextColumn("[progress.percentage]{task.percentage:>3.0f}%"),
        TimeElapsedColumn(),
        console=console,
        disable=quiet,
    ) as progress:
        task = progress.add_task("[cyan]Processing images...", total=len(images))

        with ThreadPoolExecutor(max_workers=workers) as executor:
            futures = {
                executor.submit(
                    process_image,
                    img,
                    input_dir,
                    output_dir,
                    max_size,
                    quality,
                    resume,
                ): img
                for img in images
            }

            for future in as_completed(futures):
                img = futures[future]
                try:
                    success, status = future.result()
                    if success:
                        success_count += 1
                    else:
                        fail_count += 1
                        if not quiet:
                            console.print(f"[red]Failed:[/red] {img.name} - {status}")
                except Exception as e:
                    fail_count += 1
                    if not quiet:
                        console.print(f"[red]Error:[/red] {img.name} - {e}")
                finally:
                    progress.update(task, advance=1)

    # Summary
    if not quiet:
        console.print(f"\n[bold green]Complete![/bold green]")
        console.print(f"Success: {success_count}")
        if fail_count > 0:
            console.print(f"[red]Failed: {fail_count}[/red]")

    if fail_count > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()

