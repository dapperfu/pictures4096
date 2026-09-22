//! Image extension filters used during folder discovery.

use std::path::Path;

/// Lowercase extensions the decoder stack can ingest.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "jfif", "png", "gif", "bmp", "tif", "tiff", "webp", "ico", "ppm", "pgm", "pbm", "avif", "heic",
    "heif", "hif",
];

/// File extension used for AV1 / AVIF outputs.
pub const AVIF_EXTENSION: &str = "avif";

/// Returns whether `path` is an AVIF or HEIF/HEIC output name.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use pictures4096::formats::is_heif_output;
///
/// assert!(is_heif_output(Path::new("shot.avif")));
/// assert!(is_heif_output(Path::new("shot.AVIF")));
/// assert!(!is_heif_output(Path::new("shot.jpg")));
/// ```
#[must_use]
pub fn is_heif_output(path: &Path) -> bool {
    matches!(extension_lower(path).as_deref(), Some("heic" | "heif" | "hif" | "avif"))
}

/// Returns whether `path` has a supported image extension.
///
/// # Arguments
///
/// * `path` - Candidate file path
/// * `format_filter` - Optional lowercase extension such as `jpg` or `.jpg`
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use pictures4096::formats::is_supported_image;
///
/// assert!(is_supported_image(Path::new("a.JPG"), None));
/// assert!(is_supported_image(Path::new("a.png"), Some(".png")));
/// assert!(!is_supported_image(Path::new("a.txt"), None));
/// ```
#[must_use]
pub fn is_supported_image(path: &Path, format_filter: Option<&str>) -> bool {
    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    let ext = ext.to_ascii_lowercase();
    if let Some(filter) = format_filter {
        let filter = filter.trim_start_matches('.').to_ascii_lowercase();
        return ext == filter;
    }
    SUPPORTED_EXTENSIONS.contains(&ext.as_str())
}

/// Returns the lowercase extension without a leading dot.
///
/// # Arguments
///
/// * `path` - File path
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use pictures4096::formats::extension_lower;
///
/// assert_eq!(extension_lower(Path::new("photo.JPEG")).as_deref(), Some("jpeg"));
/// ```
#[must_use]
pub fn extension_lower(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn filter_matches_dotted_and_bare_extensions() {
        assert!(is_supported_image(Path::new("x.jpg"), Some(".JPG")));
        assert!(is_supported_image(Path::new("x.jpg"), Some("jpg")));
        assert!(!is_supported_image(Path::new("x.png"), Some("jpg")));
    }
}
