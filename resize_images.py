#!/usr/bin/env python3
"""Batch image resizer with EXIF preservation for archival purposes."""

import logging
import os
import shutil
import signal
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import List, Optional, Tuple

import click
from PIL import Image, ImageFile
from rich.console import Console
from rich.logging import RichHandler
from rich.progress import Progress, SpinnerColumn, BarColumn, TextColumn, TimeElapsedColumn

# Enable loading truncated images (some images may be slightly corrupted but still usable)
ImageFile.LOAD_TRUNCATED_IMAGES = True

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format="%(message)s",
    datefmt="[%X]",
    handlers=[RichHandler(rich_tracebacks=True)],
)
logger = logging.getLogger(__name__)

console = Console()

# Global flag for graceful shutdown
_interrupted = False


def signal_handler(signum, frame):  # noqa: ARG001
    """
    Handle interrupt signals (Ctrl+C) gracefully.

    Parameters
    ----------
    signum : int
        Signal number
    frame : frame
        Current stack frame
    """
    global _interrupted
    if not _interrupted:
        _interrupted = True
        # Use print instead of console.print to avoid potential issues in signal handler
        print("\nInterrupt received. Finishing current tasks and shutting down gracefully...", file=sys.stderr)

# Check if exiftool is available
_EXIFTOOL_AVAILABLE: Optional[bool] = None
_EXIFTOOL_PATH: Optional[str] = None


def check_exiftool() -> Tuple[bool, Optional[str]]:
    """
    Check if exiftool is available on the system.

    Returns
    -------
    Tuple[bool, Optional[str]]
        (is_available, exiftool_path)
    """
    global _EXIFTOOL_AVAILABLE, _EXIFTOOL_PATH

    if _EXIFTOOL_AVAILABLE is not None:
        return _EXIFTOOL_AVAILABLE, _EXIFTOOL_PATH

    # Check if exiftool is in PATH
    exiftool_path = shutil.which("exiftool")
    if exiftool_path:
        _EXIFTOOL_AVAILABLE = True
        _EXIFTOOL_PATH = exiftool_path
        return True, exiftool_path

    # Try common alternative names
    for name in ["exiftool", "exiftool.exe"]:
        exiftool_path = shutil.which(name)
        if exiftool_path:
            _EXIFTOOL_AVAILABLE = True
            _EXIFTOOL_PATH = exiftool_path
            return True, exiftool_path

    _EXIFTOOL_AVAILABLE = False
    _EXIFTOOL_PATH = None
    return False, None


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
            # Load image data to verify it's readable
            img.load()
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

    Handles truncated and corrupted images gracefully by attempting to load
    and save what can be recovered.

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
        # Try to load the image (LOAD_TRUNCATED_IMAGES is enabled globally)
        with Image.open(source_path) as img:
            # Load the image data to verify it's readable
            img.load()
            
            width, height = img.size

            # If image is smaller than max_size, just copy it
            if width <= max_size and height <= max_size:
                # Try to preserve EXIF if available
                exif_data = img.info.get("exif")
                try:
                    img.save(output_path, quality=quality, exif=exif_data)
                except Exception as save_error:
                    # If saving with EXIF fails, try without
                    logger.warning(
                        f"Failed to save with EXIF for {source_path}, trying without: {save_error}"
                    )
                    img.save(output_path, quality=quality)
                return True

            # Resize preserving aspect ratio
            img.thumbnail((max_size, max_size), Image.Resampling.LANCZOS)
            # Try to preserve EXIF if available
            exif_data = img.info.get("exif")
            try:
                img.save(output_path, quality=quality, exif=exif_data)
            except Exception as save_error:
                # If saving with EXIF fails, try without
                logger.warning(
                    f"Failed to save with EXIF for {source_path}, trying without: {save_error}"
                )
                img.save(output_path, quality=quality)
            return True

    except Image.UnidentifiedImageError:
        logger.warning(
            f"Cannot identify image file (may be corrupted): {source_path}"
        )
        return False
    except Exception as e:
        error_msg = str(e).lower()
        if "truncated" in error_msg or "broken" in error_msg or "corrupt" in error_msg:
            logger.warning(
                f"Corrupted/truncated image (skipping): {source_path} - {e}"
            )
        else:
            logger.error(f"Failed to resize {source_path}: {e}")
        return False


def copy_exif(source_path: Path, output_path: Path) -> bool:
    """
    Copy EXIF metadata from source to output image using exiftool.

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
    # Check if exiftool is available
    exiftool_available, exiftool_path = check_exiftool()
    if not exiftool_available or not exiftool_path:
        logger.warning(
            f"exiftool not found in PATH. EXIF metadata will not be copied. "
            f"Please install exiftool: https://exiftool.org/"
        )
        return False

    try:
        # Use exiftool's -tagsFromFile to copy all metadata
        # -all:all copies all metadata tags from source to destination
        # -overwrite_original avoids creating backup files
        # -q (quiet) suppresses normal informational messages
        result = subprocess.run(
            [
                exiftool_path,
                "-tagsFromFile",
                str(source_path),
                "-all:all",
                "-overwrite_original",
                "-q",  # Quiet mode
                str(output_path),
            ],
            capture_output=True,
            text=True,
            timeout=30,  # 30 second timeout
        )

        if result.returncode == 0:
            return True
        else:
            logger.warning(
                f"exiftool failed for {source_path} -> {output_path}: "
                f"{result.stderr.strip() if result.stderr else 'Unknown error'}"
            )
            return False

    except subprocess.TimeoutExpired:
        logger.warning(
            f"exiftool timed out while copying EXIF from {source_path} to {output_path}"
        )
        return False
    except Exception as exiftool_error:
        logger.warning(
            f"Failed to copy EXIF from {source_path} to {output_path}: {exiftool_error}",
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
    # Register signal handlers for graceful shutdown
    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)
    
    # Reset interrupted flag (declare global here for use throughout function)
    global _interrupted
    _interrupted = False
    
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
    total_processed = 0

    try:
        with Progress(
            SpinnerColumn(),
            TextColumn("[progress.description]{task.description}"),
            BarColumn(),
            TextColumn("[progress.percentage]{task.percentage:>3.0f}%"),
            TimeElapsedColumn(),
            console=console,
            disable=quiet,
        ) as progress:
            task = progress.add_task(
                f"[cyan]Processing images... (0 / {len(images)})[/cyan]",
                total=len(images),
            )

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
                    # Check for interruption
                    if _interrupted:
                        if not quiet:
                            console.print("\n[yellow]Cancelling remaining tasks...[/yellow]")
                        # Cancel remaining futures
                        for f in futures:
                            f.cancel()
                        break

                    img = futures[future]
                    try:
                        success, status = future.result()
                        total_processed += 1
                        if success:
                            success_count += 1
                        else:
                            fail_count += 1
                            if not quiet:
                                # Print failure on a separate line (Rich will keep progress bar visible)
                                console.print(f"[red]Failed:[/red] {img.name} - {status}")
                    except Exception as e:
                        fail_count += 1
                        total_processed += 1
                        if not quiet:
                            # Print error on a separate line
                            console.print(f"[red]Error:[/red] {img.name} - {e}")
                    finally:
                        # Update progress with current count
                        progress.update(
                            task,
                            advance=1,
                            description=f"[cyan]Processing images... ({total_processed} / {len(images)})[/cyan]",
                        )

    except KeyboardInterrupt:
        # Fallback for KeyboardInterrupt if signal handler didn't catch it
        if not quiet:
            console.print("\n[yellow]Interrupt received. Shutting down...[/yellow]")
        # Set interrupted flag (already declared global in main())
        _interrupted = True

    # Summary
    if not quiet:
        if _interrupted:
            console.print(f"\n[yellow]Interrupted![/yellow]")
            console.print(f"Processed: {total_processed} of {len(images)}")
            console.print(f"Success: {success_count}")
            if fail_count > 0:
                console.print(f"[red]Failed: {fail_count}[/red]")
            console.print("[yellow]You can resume processing with the same command (resume is enabled by default)[/yellow]")
        else:
            console.print(f"\n[bold green]Complete![/bold green]")
            console.print(f"Success: {success_count}")
            if fail_count > 0:
                console.print(f"[red]Failed: {fail_count}[/red]")

    # Exit with appropriate code
    if _interrupted:
        sys.exit(130)  # Standard exit code for SIGINT
    elif fail_count > 0:
        sys.exit(1)
    else:
        sys.exit(0)


if __name__ == "__main__":
    main()

