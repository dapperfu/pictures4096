//! Integration tests for folder ingest.

use std::sync::atomic::AtomicBool;

use image::{DynamicImage, RgbImage};
use pictures4096::process::{ingest_folder, IngestOptions};
use pictures4096::resize::encode_image;
use tempfile::tempdir;

#[test]
fn ingest_preserves_relative_layout_and_resume() {
    let input = tempdir().expect("input");
    let output = tempdir().expect("output");
    let nested = input.path().join("album");
    std::fs::create_dir_all(&nested).expect("mkdir");
    let src = nested.join("shot.jpg");
    let img = DynamicImage::ImageRgb8(RgbImage::from_pixel(40, 20, image::Rgb([9, 8, 7])));
    let bytes = encode_image(&img, &src, 85).expect("encode");
    std::fs::write(&src, bytes).expect("write");

    let cancel = AtomicBool::new(false);
    let options = IngestOptions {
        max_size: 10,
        quality: 80,
        resume: true,
        format_filter: None,
        limit: None,
        workers: Some(2),
        copy_exif: false,
        exif_only: false,
    };
    let first = ingest_folder(input.path(), output.path(), &options, &cancel, None).expect("first");
    assert_eq!(first.success, 1);
    let dest = output.path().join("album/shot.jpg");
    assert!(dest.exists());
    let out = image::open(&dest).expect("open");
    assert!(out.width() <= 10);
    assert!(out.height() <= 10);

    let second = ingest_folder(input.path(), output.path(), &options, &cancel, None).expect("second");
    assert_eq!(second.skipped, 1);
    assert_eq!(second.success, 0);
}
