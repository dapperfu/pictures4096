//! Decode, SIMD resize, encode, and output validation.

use std::cell::RefCell;
use std::io::Cursor;
use std::path::Path;

use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, ResizeOptions, Resizer};
use image::codecs::avif::{AvifEncoder, ColorSpace};
use image::codecs::jpeg::JpegEncoder;
use image::{ColorType, DynamicImage, GenericImageView, ImageEncoder, ImageFormat, ImageReader};

use crate::error::Error;
use crate::formats::{extension_lower, is_heif_output};
use crate::geometry::fit_dimensions;

thread_local! {
    static RESIZER: RefCell<Resizer> = RefCell::new(Resizer::new());
}

/// Default JPEG / AVIF quality (1-100).
pub const DEFAULT_QUALITY: u8 = 95;

/// Default rav1e speed (1 = smallest/slowest, 10 = fastest). Low = better compression.
pub const DEFAULT_AVIF_SPEED: u8 = 2;

/// Resize `source` into `output` so both edges fit in `max_size`.
///
/// JPEGs keep source `APPn` metadata. Other formats are re-encoded with the
/// original container when the `image` crate supports writing it.
///
/// # Arguments
///
/// * `source` - Input image path
/// * `output` - Destination path (parent should already exist)
/// * `max_size` - Maximum edge length in pixels
/// * `quality` - JPEG quality 1-100
/// * `copy_exif` - Copy source EXIF onto the output with fast-exif-rs
/// * `avif_speed` - rav1e speed 1-10 (ignored unless `output` is `.avif`)
///
/// # Errors
///
/// Returns decode, resize, or encode failures.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use pictures4096::resize::resize_image;
///
/// let _ = resize_image(Path::new("in.jpg"), Path::new("out.avif"), 4096, 95, true, 2);
/// ```
pub fn resize_image(
    source: &Path,
    output: &Path,
    max_size: u32,
    quality: u8,
    copy_exif: bool,
    avif_speed: u8,
) -> Result<(), Error> {
    let bytes = std::fs::read(source)?;
    let image = decode_image(&bytes, source)?;
    let resized = resize_dynamic(&image, max_size)?;
    let encoded = if is_heif_output(output) {
        encode_avif(&resized, quality, avif_speed)?
    } else {
        encode_image(&resized, source, quality)?
    };
    std::fs::write(output, &encoded)?;
    if copy_exif && crate::exif::copy_exif(source, output, output).is_err() {
        if let Ok(merged) = crate::exif::copy_exif_bytes(&bytes, &encoded) {
            std::fs::write(output, merged)?;
        }
    }
    validate_image(output)?;
    Ok(())
}

/// Decodes image bytes, using `path` only for error context and format hints.
///
/// # Errors
///
/// Returns [`Error::Decode`] when the payload is not a readable image.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use pictures4096::resize::decode_image;
///
/// assert!(decode_image(b"not-an-image", Path::new("x.jpg")).is_err());
/// ```
pub fn decode_image(bytes: &[u8], path: &Path) -> Result<DynamicImage, Error> {
    let mut reader = ImageReader::new(Cursor::new(bytes));
    if let Some(format) = format_from_path(path) {
        reader.set_format(format);
    } else {
        reader = reader
            .with_guessed_format()
            .map_err(|error| Error::Decode(error.to_string()))?;
    }
    reader.decode().map_err(|error| Error::Decode(error.to_string()))
}

/// Fits `image` into `max_size` using Lanczos3 convolution.
///
/// # Errors
///
/// Returns [`Error::Resize`] if the SIMD resizer rejects the buffers.
///
/// # Examples
///
/// ```
/// use image::{DynamicImage, RgbImage};
/// use pictures4096::resize::resize_dynamic;
///
/// let src = DynamicImage::ImageRgb8(RgbImage::new(10, 10));
/// let out = resize_dynamic(&src, 4096).expect("resize");
/// assert_eq!(out.width(), 10);
/// ```
pub fn resize_dynamic(image: &DynamicImage, max_size: u32) -> Result<DynamicImage, Error> {
    let (width, height) = image.dimensions();
    let (new_width, new_height) = fit_dimensions(width, height, max_size);
    if new_width == width && new_height == height {
        return Ok(image.clone());
    }

    let rgb = image.to_rgb8();
    let src = Image::from_vec_u8(width, height, rgb.into_raw(), PixelType::U8x3)
        .map_err(|error| Error::Resize(error.to_string()))?;
    let mut dst = Image::new(new_width, new_height, PixelType::U8x3);
    let options = ResizeOptions::new().resize_alg(ResizeAlg::Convolution(FilterType::Lanczos3));
    RESIZER.with(|resizer| {
        resizer
            .borrow_mut()
            .resize(&src, &mut dst, &options)
            .map_err(|error| Error::Resize(error.to_string()))
    })?;

    let buffer = dst.buffer().to_vec();
    let rgb = image::RgbImage::from_raw(new_width, new_height, buffer)
        .ok_or_else(|| Error::Resize("invalid resized RGB buffer".to_owned()))?;
    Ok(DynamicImage::ImageRgb8(rgb))
}

/// Encodes `image` using the source path extension (JPEG quality for JPEG).
///
/// # Errors
///
/// Returns [`Error::Encode`] when the encoder fails.
///
/// # Examples
///
/// ```
/// use image::{DynamicImage, RgbImage};
/// use std::path::Path;
/// use pictures4096::resize::encode_image;
///
/// let img = DynamicImage::ImageRgb8(RgbImage::new(2, 2));
/// let bytes = encode_image(&img, Path::new("x.jpg"), 90).expect("encode");
/// assert!(bytes.starts_with(&[0xFF, 0xD8]));
/// ```
pub fn encode_image(image: &DynamicImage, source: &Path, quality: u8) -> Result<Vec<u8>, Error> {
    let quality = quality.clamp(1, 100);
    let mut encoded = Vec::new();
    match format_from_path(source).unwrap_or(ImageFormat::Jpeg) {
        ImageFormat::Jpeg => {
            let rgb = image.to_rgb8();
            let jpeg = JpegEncoder::new_with_quality(&mut encoded, quality);
            jpeg.write_image(rgb.as_raw(), rgb.width(), rgb.height(), ColorType::Rgb8.into())
                .map_err(|error| Error::Encode(error.to_string()))?;
        }
        ImageFormat::Png => {
            image
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Png)
                .map_err(|error| Error::Encode(error.to_string()))?;
        }
        ImageFormat::WebP => {
            image
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::WebP)
                .map_err(|error| Error::Encode(error.to_string()))?;
        }
        ImageFormat::Gif => {
            image
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Gif)
                .map_err(|error| Error::Encode(error.to_string()))?;
        }
        ImageFormat::Tiff => {
            image
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Tiff)
                .map_err(|error| Error::Encode(error.to_string()))?;
        }
        ImageFormat::Bmp => {
            image
                .write_to(&mut Cursor::new(&mut encoded), ImageFormat::Bmp)
                .map_err(|error| Error::Encode(error.to_string()))?;
        }
        other => {
            image
                .write_to(&mut Cursor::new(&mut encoded), other)
                .map_err(|error| Error::Encode(error.to_string()))?;
        }
    }
    Ok(encoded)
}

/// Encodes `image` as AVIF (AV1 in a HEIF container).
///
/// Speed 1 spends the most CPU for the smallest file. Encoding uses one rav1e
/// thread so folder-level Rayon can own parallelism.
///
/// # Errors
///
/// Returns [`Error::Encode`] when ravif fails.
///
/// # Examples
///
/// ```
/// use image::{DynamicImage, RgbImage};
/// use pictures4096::resize::encode_avif;
///
/// let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(4, 4, image::Rgb([1, 2, 3])));
/// let bytes = encode_avif(&img, 60, 10).expect("avif");
/// assert!(bytes.windows(4).any(|chunk| chunk == b"ftyp"));
/// ```
pub fn encode_avif(image: &DynamicImage, quality: u8, speed: u8) -> Result<Vec<u8>, Error> {
    let quality = quality.clamp(1, 100);
    let speed = speed.clamp(1, 10);
    let rgba = image.to_rgba8();
    let mut encoded = Vec::new();
    let avif = AvifEncoder::new_with_speed_quality(&mut encoded, speed, quality)
        .with_colorspace(ColorSpace::Bt709)
        .with_num_threads(Some(1));
    avif.write_image(rgba.as_raw(), rgba.width(), rgba.height(), ColorType::Rgba8.into())
        .map_err(|error| Error::Encode(error.to_string()))?;
    Ok(encoded)
}

/// Verifies that `path` decodes as an image.
///
/// # Errors
///
/// Returns [`Error::Encode`] when the file is unreadable or not an image.
///
/// # Examples
///
/// ```
/// use pictures4096::resize::validate_image;
/// use std::path::Path;
///
/// assert!(validate_image(Path::new("/no/such/file.jpg")).is_err());
/// ```
pub fn validate_image(path: &Path) -> Result<(), Error> {
    if is_heif_output(path) {
        return validate_heif_container(path);
    }
    ImageReader::open(path)
        .map_err(|error| Error::Encode(error.to_string()))?
        .with_guessed_format()
        .map_err(|error| Error::Encode(error.to_string()))?
        .decode()
        .map_err(|error| Error::Encode(error.to_string()))?;
    Ok(())
}

fn validate_heif_container(path: &Path) -> Result<(), Error> {
    let bytes = std::fs::read(path)?;
    if bytes.windows(4).any(|chunk| chunk == b"ftyp") {
        return Ok(());
    }
    Err(Error::Encode(format!(
        "output is not a HEIF/AVIF container: {}",
        path.display()
    )))
}

fn format_from_path(path: &Path) -> Option<ImageFormat> {
    match extension_lower(path)?.as_str() {
        "jpg" | "jpeg" | "jfif" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "gif" => Some(ImageFormat::Gif),
        "bmp" => Some(ImageFormat::Bmp),
        "tif" | "tiff" => Some(ImageFormat::Tiff),
        "webp" => Some(ImageFormat::WebP),
        "ico" => Some(ImageFormat::Ico),
        "avif" | "heic" | "heif" | "hif" => Some(ImageFormat::Avif),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbImage;
    use tempfile::tempdir;

    #[test]
    fn round_trip_jpeg_fits_max_size() {
        let dir = tempdir().expect("tempdir");
        let source = dir.path().join("big.jpg");
        let dest = dir.path().join("out.jpg");
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(64, 32, image::Rgb([10, 20, 30])));
        let bytes = encode_image(&img, &source, 90).expect("encode src");
        std::fs::write(&source, bytes).expect("write src");
        resize_image(&source, &dest, 16, 80, false, 10).expect("resize");
        let out = image::open(&dest).expect("open dest");
        assert!(out.width() <= 16);
        assert!(out.height() <= 16);
    }

    #[test]
    fn avif_round_trip() {
        let dir = tempdir().expect("tempdir");
        let source = dir.path().join("big.jpg");
        let dest = dir.path().join("out.avif");
        let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(24, 16, image::Rgb([40, 80, 120])));
        let bytes = encode_image(&img, &source, 90).expect("encode src");
        std::fs::write(&source, bytes).expect("write src");
        resize_image(&source, &dest, 12, 50, false, 10).expect("avif");
        assert!(dest.exists());
        let raw = std::fs::read(&dest).expect("read avif");
        assert!(raw.windows(4).any(|chunk| chunk == b"ftyp"));
        assert!(raw
            .windows(4)
            .any(|chunk| chunk == b"avif" || chunk == b"avis" || chunk == b"mif1"));
    }
}
