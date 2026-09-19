//! Example: resize a folder with the library API.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use pictures4096::process::{ingest_folder, IngestOptions};

fn main() {
    let input = std::env::args().nth(1).unwrap_or_else(|| ".".to_owned());
    let output = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "/tmp/pictures4096-example".to_owned());
    let cancel = AtomicBool::new(false);
    match ingest_folder(
        Path::new(&input),
        Path::new(&output),
        &IngestOptions {
            format_filter: Some(".nope".to_owned()),
            ..IngestOptions::default()
        },
        &cancel,
        None,
    ) {
        Ok(stats) => println!("processed {}", stats.total()),
        Err(error) => println!("expected empty ingest: {error}"),
    }
}
