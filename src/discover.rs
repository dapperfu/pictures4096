//! Recursive discovery of image files.

use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::error::Error;
use crate::formats::is_supported_image;

/// Recursively finds image files under `input_dir`, sorted for stable processing.
///
/// # Arguments
///
/// * `input_dir` - Root folder to walk
/// * `format_filter` - Optional extension filter such as `.jpg`
///
/// # Errors
///
/// Returns [`Error::Io`] when the directory cannot be walked.
///
/// # Examples
///
/// ```
/// use pictures4096::discover::discover_images;
/// use std::path::Path;
///
/// let images = discover_images(Path::new("."), Some(".nope")).expect("walk current dir");
/// assert!(images.is_empty());
/// ```
pub fn discover_images(input_dir: &Path, format_filter: Option<&str>) -> Result<Vec<PathBuf>, Error> {
    if !input_dir.is_dir() {
        return Err(Error::InvalidInput(format!(
            "input path is not a directory: {}",
            input_dir.display()
        )));
    }

    let mut images = Vec::new();
    let walker = WalkDir::new(input_dir).follow_links(false).into_iter();
    for entry in walker {
        let entry = entry.map_err(|error| Error::Io(error.to_string()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.into_path();
        if is_supported_image(&path, format_filter) {
            images.push(path);
        }
    }
    images.sort();
    Ok(images)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn finds_nested_jpeg_and_ignores_text() {
        let dir = tempdir().expect("tempdir");
        let nested = dir.path().join("nested");
        fs::create_dir_all(&nested).expect("mkdir");
        fs::write(nested.join("a.jpg"), b"not-a-real-jpeg").expect("write jpg");
        fs::write(dir.path().join("notes.txt"), b"hello").expect("write txt");

        let found = discover_images(dir.path(), None).expect("discover");
        assert_eq!(found.len(), 1);
        assert!(found[0].ends_with("a.jpg"));
    }

    #[test]
    fn rejects_non_directory() {
        let dir = tempdir().expect("tempdir");
        let file = dir.path().join("file.txt");
        fs::write(&file, b"x").expect("write");
        assert!(discover_images(&file, None).is_err());
    }
}
