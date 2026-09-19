//! Folder ingest: map input paths onto output paths and process in parallel.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use rayon::prelude::*;

use crate::discover::discover_images;
use crate::error::Error;
use crate::resize::resize_image;

/// Options for a batch ingest/resize run.
#[derive(Debug, Clone)]
pub struct IngestOptions {
    /// Maximum output edge length.
    pub max_size: u32,
    /// JPEG quality 1-100.
    pub quality: u8,
    /// Skip files whose output already exists.
    pub resume: bool,
    /// Optional extension filter.
    pub format_filter: Option<String>,
    /// Optional cap on how many images to process after discovery.
    pub limit: Option<usize>,
    /// Rayon worker count; `None` uses every logical CPU.
    pub workers: Option<usize>,
    /// Copy EXIF with fast-exif-rs (default on).
    pub copy_exif: bool,
    /// Only copy EXIF onto existing outputs (no resize).
    pub exif_only: bool,
}

impl Default for IngestOptions {
    fn default() -> Self {
        Self {
            max_size: crate::geometry::DEFAULT_MAX_SIZE,
            quality: crate::resize::DEFAULT_QUALITY,
            resume: true,
            format_filter: None,
            limit: None,
            workers: None,
            copy_exif: true,
            exif_only: false,
        }
    }
}

/// Outcome of one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileStatus {
    /// Resized (or re-encoded) successfully.
    Success,
    /// Skipped because resume found an existing output.
    Skipped,
    /// Failed with a short reason.
    Failed(String),
}

/// Counters collected during a batch.
#[derive(Debug, Default, Clone, Copy)]
pub struct BatchStats {
    /// Successfully written images.
    pub success: u64,
    /// Resume skips.
    pub skipped: u64,
    /// Failures.
    pub failed: u64,
}

impl BatchStats {
    /// Total files that reached a terminal status.
    ///
    /// # Examples
    ///
    /// ```
    /// use pictures4096::process::BatchStats;
    ///
    /// let stats = BatchStats { success: 2, skipped: 1, failed: 1 };
    /// assert_eq!(stats.total(), 4);
    /// ```
    #[must_use]
    pub const fn total(self) -> u64 {
        self.success + self.skipped + self.failed
    }
}

/// Progress callback: `(completed, total, path, status)`.
pub type ProgressFn = Arc<dyn Fn(u64, u64, &Path, &FileStatus) + Send + Sync>;

/// Discovers images and resizes them into `output_dir`, preserving relative paths.
///
/// # Arguments
///
/// * `input_dir` - Folder to ingest
/// * `output_dir` - Destination root
/// * `options` - Size, quality, resume, workers
/// * `cancel` - Cooperative cancellation flag
/// * `on_progress` - Optional per-file progress hook
///
/// # Errors
///
/// Returns discovery errors. Per-file failures are counted in [`BatchStats`].
///
/// # Examples
///
/// ```
/// use pictures4096::process::{ingest_folder, IngestOptions};
/// use std::path::Path;
/// use std::sync::atomic::AtomicBool;
///
/// let cancel = AtomicBool::new(false);
/// let result = ingest_folder(Path::new("."), Path::new("/tmp/pictures4096-doctest"), &IngestOptions {
///     format_filter: Some(".nope".to_owned()),
///     ..IngestOptions::default()
/// }, &cancel, None);
/// assert!(result.is_err() || result.unwrap().total() == 0);
/// ```
pub fn ingest_folder(
    input_dir: &Path,
    output_dir: &Path,
    options: &IngestOptions,
    cancel: &AtomicBool,
    on_progress: Option<&ProgressFn>,
) -> Result<BatchStats, Error> {
    let filter = options.format_filter.as_deref();
    let mut images = discover_images(input_dir, filter)?;
    if let Some(limit) = options.limit {
        images.truncate(limit);
    }
    if images.is_empty() {
        return Err(Error::InvalidInput("no images found".to_owned()));
    }

    std::fs::create_dir_all(output_dir)?;

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(worker_count(options.workers))
        .build()
        .map_err(|error| Error::Io(error.to_string()))?;

    let success = AtomicU64::new(0);
    let skipped = AtomicU64::new(0);
    let failed = AtomicU64::new(0);
    let completed = AtomicU64::new(0);
    let total = u64::try_from(images.len()).unwrap_or(u64::MAX);
    let input_dir = input_dir.to_path_buf();
    let output_dir = output_dir.to_path_buf();

    pool.install(|| {
        images.par_iter().for_each(|source| {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let outcome = process_one(source, &input_dir, &output_dir, options);
            match &outcome {
                FileStatus::Success => {
                    success.fetch_add(1, Ordering::Relaxed);
                }
                FileStatus::Skipped => {
                    skipped.fetch_add(1, Ordering::Relaxed);
                }
                FileStatus::Failed(_) => {
                    failed.fetch_add(1, Ordering::Relaxed);
                }
            }
            let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
            if let Some(callback) = on_progress {
                callback(done, total, source, &outcome);
            }
        });
    });

    Ok(BatchStats {
        success: success.load(Ordering::Relaxed),
        skipped: skipped.load(Ordering::Relaxed),
        failed: failed.load(Ordering::Relaxed),
    })
}

/// Processes a single image into the mirrored output path.
///
/// # Arguments
///
/// * `source` - Source file
/// * `input_dir` - Ingest root
/// * `output_dir` - Output root
/// * `options` - Resize options
///
/// # Examples
///
/// ```
/// use pictures4096::process::{process_one, IngestOptions};
/// use std::path::Path;
///
/// let status = process_one(
///     Path::new("/no/such.jpg"),
///     Path::new("/no"),
///     Path::new("/tmp"),
///     &IngestOptions::default(),
/// );
/// assert!(matches!(status, pictures4096::process::FileStatus::Failed(_)));
/// ```
#[must_use]
pub fn process_one(source: &Path, input_dir: &Path, output_dir: &Path, options: &IngestOptions) -> FileStatus {
    let rel = match source.strip_prefix(input_dir) {
        Ok(path) => path.to_path_buf(),
        Err(_) => PathBuf::from(source.file_name().unwrap_or_default()),
    };
    let dest = output_dir.join(rel);
    if options.resume && dest.exists() && !options.exif_only {
        return FileStatus::Skipped;
    }
    if let Some(parent) = dest.parent() {
        if let Err(error) = std::fs::create_dir_all(parent) {
            return FileStatus::Failed(error.to_string());
        }
    }
    if options.exif_only {
        if !dest.exists() {
            return FileStatus::Failed("output missing for --exif-only".to_owned());
        }
        return match crate::exif::copy_exif(source, &dest, &dest) {
            Ok(()) => FileStatus::Success,
            Err(error) => FileStatus::Failed(error.to_string()),
        };
    }
    match resize_image(source, &dest, options.max_size, options.quality, options.copy_exif) {
        Ok(()) => FileStatus::Success,
        Err(error) => FileStatus::Failed(error.to_string()),
    }
}

/// Resolves the worker count, defaulting to every logical CPU (at least 1).
///
/// # Examples
///
/// ```
/// use pictures4096::process::worker_count;
///
/// assert_eq!(worker_count(Some(4)), 4);
/// assert!(worker_count(None) >= 1);
/// ```
#[must_use]
pub fn worker_count(requested: Option<usize>) -> usize {
    requested
        .filter(|count| *count > 0)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, RgbImage};
    use tempfile::tempdir;

    #[test]
    fn ingest_writes_fitted_jpeg() {
        let input = tempdir().expect("in");
        let output = tempdir().expect("out");
        let src = input.path().join("p.jpg");
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(32, 16, image::Rgb([1, 2, 3])));
        let bytes = crate::resize::encode_image(&img, &src, 90).expect("enc");
        std::fs::write(&src, bytes).expect("write");

        let cancel = AtomicBool::new(false);
        let stats = ingest_folder(
            input.path(),
            output.path(),
            &IngestOptions {
                max_size: 8,
                quality: 80,
                resume: false,
                format_filter: None,
                limit: None,
                workers: Some(2),
                copy_exif: false,
                exif_only: false,
            },
            &cancel,
            None,
        )
        .expect("ingest");
        assert_eq!(stats.success, 1);
        assert!(output.path().join("p.jpg").exists());
    }
}
