//! Batch image ingest and resize with aspect-ratio fitting and JPEG EXIF copy.
//!
//! # Examples
//!
//! ```
//! use pictures4096::geometry::fit_dimensions;
//!
//! assert_eq!(fit_dimensions(8192, 4096, 4096), (4096, 2048));
//! ```

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]

pub mod discover;
pub mod error;
pub mod exif;
pub mod formats;
pub mod geometry;
pub mod jpeg_exif;
pub mod process;
pub mod resize;

pub use error::Error;
pub use process::{ingest_folder, BatchStats, FileStatus, IngestOptions};
