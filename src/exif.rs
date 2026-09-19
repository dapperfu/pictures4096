//! Copy camera metadata with [`fast_exif_reader`](https://github.com/dapperfu/fast-exif-rs).

use std::cell::RefCell;
use std::path::Path;

use fast_exif_reader::FastExifCopier;

use crate::error::Error;
use crate::jpeg_exif::{is_jpeg, merge_jpeg_metadata};

thread_local! {
    static COPIER: RefCell<FastExifCopier> = RefCell::new(FastExifCopier::new());
}

/// Copies EXIF from `source` onto `target`, writing `output`.
///
/// Tries `fast-exif-rs` all-tags copy, then its high-priority subset, then a
/// JPEG `APPn` merge when both files are JPEGs. An empty source tag set is
/// treated as success (nothing to copy).
///
/// # Arguments
///
/// * `source` - File with the original metadata
/// * `target` - Image that should receive tags (often the resized file)
/// * `output` - Destination path (may equal `target`)
///
/// # Errors
///
/// Returns [`Error::Exif`] or [`Error::Io`] when every copy strategy fails.
///
/// # Examples
///
/// ```
/// use pictures4096::exif::copy_exif;
/// use std::path::Path;
///
/// assert!(copy_exif(Path::new("/no/src.jpg"), Path::new("/no/dst.jpg"), Path::new("/no/out.jpg")).is_err());
/// ```
pub fn copy_exif(source: &Path, target: &Path, output: &Path) -> Result<(), Error> {
    let source_s = path_utf8(source)?;
    let target_s = path_utf8(target)?;
    let output_s = path_utf8(output)?;

    let all_result = COPIER.with(|copier| copier.borrow_mut().copy_all_exif(source_s, target_s, output_s));
    if all_result.is_ok() {
        return Ok(());
    }

    let high_result = COPIER.with(|copier| {
        copier
            .borrow_mut()
            .copy_high_priority_exif(source_s, target_s, output_s)
    });
    if high_result.is_ok() {
        return Ok(());
    }

    if let Some(merged) = jpeg_appn_fallback(source, target)? {
        std::fs::write(output, merged)?;
        return Ok(());
    }

    let detail = match (all_result.err(), high_result.err()) {
        (Some(first), Some(second)) => format!("{first}; fallback: {second}"),
        (Some(first), None) => first.to_string(),
        (None, Some(second)) => second.to_string(),
        (None, None) => "unknown EXIF copy failure".to_owned(),
    };
    if is_empty_source(&detail) {
        return Ok(());
    }
    Err(Error::Exif(detail))
}

/// Copies EXIF from `source_bytes` onto `target_bytes` (high-priority subset).
///
/// # Arguments
///
/// * `source_bytes` - Original file bytes
/// * `target_bytes` - Encoded image bytes
///
/// # Errors
///
/// Returns [`Error::Exif`] when the crate cannot write tags and JPEG merge fails.
///
/// # Examples
///
/// ```
/// use pictures4096::exif::copy_exif_bytes;
///
/// let tiny = [0xFF, 0xD8, 0xFF, 0xD9];
/// let _ = copy_exif_bytes(&tiny, &tiny);
/// ```
pub fn copy_exif_bytes(source_bytes: &[u8], target_bytes: &[u8]) -> Result<Vec<u8>, Error> {
    let result = COPIER.with(|copier| {
        copier
            .borrow_mut()
            .copy_high_priority_exif_to_bytes(source_bytes, target_bytes)
    });
    match result {
        Ok(bytes) => Ok(bytes),
        Err(error) if is_empty_source(&error.to_string()) => Ok(target_bytes.to_vec()),
        Err(error) => {
            if is_jpeg(source_bytes) && is_jpeg(target_bytes) {
                return Ok(merge_jpeg_metadata(source_bytes, target_bytes));
            }
            Err(Error::Exif(error.to_string()))
        }
    }
}

fn jpeg_appn_fallback(source: &Path, target: &Path) -> Result<Option<Vec<u8>>, Error> {
    let source_bytes = std::fs::read(source)?;
    let target_bytes = std::fs::read(target)?;
    if is_jpeg(&source_bytes) && is_jpeg(&target_bytes) {
        return Ok(Some(merge_jpeg_metadata(&source_bytes, &target_bytes)));
    }
    Ok(None)
}

fn path_utf8(path: &Path) -> Result<&str, Error> {
    path.to_str()
        .ok_or_else(|| Error::InvalidInput(format!("non-UTF-8 path: {}", path.display())))
}

fn is_empty_source(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("no exif") || lower.contains("no high-priority")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_jpeg_copy_keeps_target_bytes() {
        let tiny = [0xFF, 0xD8, 0xFF, 0xD9];
        let out = copy_exif_bytes(&tiny, &tiny).expect("copy");
        assert_eq!(out, tiny);
    }
}
