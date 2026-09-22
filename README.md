# pictures4096

I shoot on a Nikon. The raw library is past 4TB. Two jobs matter: get a copy onto Backblaze without paying to store every original pixel, and keep a second copy on cheap 2–4TB drives that actually fill. The masters stay put; this makes the version you can afford to ship off-site and leave on a spare disk.

A year of 20+ megapixel JPEGs is already too big to browse as the everyday copy. I do not want a sloppy downscale that throws away date, camera, and the folder layout I already use.

4K is 3840 wide. Capping the long edge at 4096 keeps images sharp on a 4K screen (and a bit of headroom) while cutting most of the bulk. Outputs are `.avif` (AV1), much smaller than JPEG at the same quality. That is the tool: take a photo tree, write a parallel tree of 4096-fit AVIF files, keep EXIF, land at a size Backblaze and a 2–4TB drive can swallow.

Python + Pillow + forking `exiftool` on every file works. It does not stay fun past a couple thousand frames. This rewrite is Rust, all cores, Lanczos resize, EXIF via [fast-exif-rs](https://github.com/dapperfu/fast-exif-rs) instead of Perl per image. Same job, less waiting.

Nothing is upscaled. Smaller shots are left at their size (re-encoded). Existing outputs are skipped so you can Ctrl-C and run the same command again.

## Install

Needs a Rust toolchain (`rustc` / `cargo`). Then:

```bash
make install
```

That builds a release binary and puts it at `~/.local/bin/pictures4096`. Put `~/.local/bin` on your `PATH` if it is not already.

```bash
pictures4096 --help
```

## Use

```bash
pictures4096 ~/Desktop/Pictures ~/Pictures/4096
```

Walks the input folder, writes the same relative paths as `.avif` (speed 2 by default — slower, smaller). Long edge ≤ 4096, quality 95, EXIF copied. Failures print and the rest keep going.

```bash
pictures4096 --max-size 2048 --quality 80 --avif-speed 1 photos photos_avif
pictures4096 --keep-format in out           # leave JPEG/PNG as JPEG/PNG
pictures4096 --no-copy-exif in out          # pixels only
pictures4096 --no-resume in out             # overwrite
pictures4096 --exif-only in out             # stamp EXIF onto files already there
```

`make bench N=100` times this against the old Python script. `make bench-exif N=100` times just the EXIF write (`exiftool` vs fast-exif-rs).
