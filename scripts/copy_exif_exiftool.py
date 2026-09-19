#!/usr/bin/env python3
"""Copy EXIF from a source tree onto existing outputs using exiftool."""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path


def copy_one(exiftool: str, source: Path, dest: Path) -> tuple[Path, bool, str]:
    if not dest.exists():
        return dest, False, "output missing"
    result = subprocess.run(
        [
            exiftool,
            "-tagsFromFile",
            str(source),
            "-all:all",
            "-overwrite_original",
            "-q",
            str(dest),
        ],
        capture_output=True,
        text=True,
        timeout=30,
        check=False,
    )
    if result.returncode == 0:
        return dest, True, "ok"
    return dest, False, result.stderr.strip() or "exiftool failed"


def main() -> int:
    if len(sys.argv) < 3:
        print("Usage: copy_exif_exiftool.py INPUT_DIR OUTPUT_DIR", file=sys.stderr)
        return 2
    input_dir = Path(sys.argv[1])
    output_dir = Path(sys.argv[2])
    exiftool = shutil.which("exiftool")
    if not exiftool:
        print("exiftool not found in PATH", file=sys.stderr)
        return 1
    sources = sorted(path for path in input_dir.rglob("*") if path.is_file())
    workers = os.cpu_count() or 1
    ok = 0
    failed = 0
    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = []
        for source in sources:
            dest = output_dir / source.relative_to(input_dir)
            futures.append(pool.submit(copy_one, exiftool, source, dest))
        for future in as_completed(futures):
            _dest, success, status = future.result()
            if success:
                ok += 1
            else:
                failed += 1
                print(f"failed: {status}", file=sys.stderr)
    print(f"exiftool copy success: {ok}  failed: {failed}")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
