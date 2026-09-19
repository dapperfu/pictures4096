//! CLI for ingesting photo folders and writing size-capped copies.

#![deny(missing_docs)]
#![deny(warnings)]
#![deny(clippy::all)]
#![deny(clippy::pedantic)]

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use pictures4096::process::{ingest_folder, FileStatus, IngestOptions};
use pictures4096::{BatchStats, Error};

/// Batch-resize images so each edge fits a maximum size (default 4096).
#[derive(Debug, Parser)]
#[command(name = "pictures4096", version, about, long_about = None)]
struct Cli {
    /// Folder to ingest recursively
    input_dir: PathBuf,
    /// Output folder; relative paths are preserved
    output_dir: PathBuf,
    /// Maximum width or height in pixels
    #[arg(long, default_value_t = 4096)]
    max_size: u32,
    /// JPEG quality 1-100
    #[arg(long, default_value_t = 95)]
    quality: u8,
    /// Parallel workers (default: all logical CPUs)
    #[arg(long)]
    workers: Option<usize>,
    /// Reprocess images even if the output already exists
    #[arg(long)]
    no_resume: bool,
    /// Minimal output (no progress bars)
    #[arg(long)]
    quiet: bool,
    /// Only process this extension (for example `.jpg`)
    #[arg(long = "format")]
    format_filter: Option<String>,
    /// Process at most this many discovered files (sorted order)
    #[arg(long)]
    limit: Option<usize>,
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<ExitCode> {
    let cli = Cli::parse();
    if cli.max_size == 0 {
        anyhow::bail!("--max-size must be greater than 0");
    }

    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_handler = Arc::clone(&cancel);
    ctrlc::set_handler(move || {
        cancel_handler.store(true, Ordering::Relaxed);
    })
    .context("install Ctrl-C handler")?;

    let options = IngestOptions {
        max_size: cli.max_size,
        quality: cli.quality,
        resume: !cli.no_resume,
        format_filter: cli.format_filter,
        limit: cli.limit,
        workers: cli.workers,
    };

    let show_ui = !cli.quiet && std::io::stderr().is_terminal();
    let started = Instant::now();
    if !cli.quiet {
        eprintln!("pictures4096");
        eprintln!("input:    {}", cli.input_dir.display());
        eprintln!("output:   {}", cli.output_dir.display());
        eprintln!("max-size: {}px", cli.max_size);
        eprintln!("quality:  {}", cli.quality);
        eprintln!("workers:  {}", pictures4096::process::worker_count(cli.workers));
    }

    let progress = if show_ui { Some(setup_progress()) } else { None };

    let on_progress = progress.as_ref().map(|(multi, overall, success, failed)| {
        let overall = overall.clone();
        let success_bar = success.clone();
        let failed_bar = failed.clone();
        let multi = multi.clone();
        Arc::new(
            move |done: u64, total: u64, path: &std::path::Path, status: &FileStatus| {
                overall.set_length(total);
                overall.set_position(done);
                match status {
                    FileStatus::Success | FileStatus::Skipped => success_bar.inc(1),
                    FileStatus::Failed(reason) => {
                        failed_bar.inc(1);
                        let _ = multi.println(format!("failed: {} ({reason})", path.display()));
                    }
                }
            },
        ) as pictures4096::process::ProgressFn
    });

    let stats = ingest_folder(&cli.input_dir, &cli.output_dir, &options, &cancel, on_progress.as_ref());
    if let Some((_, overall, success, failed)) = progress {
        overall.finish();
        success.finish();
        failed.finish();
    }

    let stats = match stats {
        Ok(stats) => stats,
        Err(Error::InvalidInput(message)) if message.contains("no images found") => {
            if !cli.quiet {
                eprintln!("No images found.");
            }
            return Ok(ExitCode::from(1));
        }
        Err(error) => return Err(error.into()),
    };

    print_summary(cli.quiet, cancel.load(Ordering::Relaxed), stats, started.elapsed());

    if cancel.load(Ordering::Relaxed) {
        Ok(ExitCode::from(130))
    } else if stats.failed > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

fn setup_progress() -> (MultiProgress, ProgressBar, ProgressBar, ProgressBar) {
    let multi = MultiProgress::new();
    let style = ProgressStyle::with_template(
        "{prefix:12} [{bar:40.cyan/blue}] {pos}/{len} {per_sec} elapsed: {elapsed_precise} eta: {eta}",
    )
    .unwrap_or_else(|_| ProgressStyle::default_bar())
    .progress_chars("=>-");
    let overall = multi.add(ProgressBar::new(0));
    overall.set_style(style.clone());
    overall.set_prefix("resize");
    let success = multi.add(ProgressBar::new(0));
    success.set_style(
        ProgressStyle::with_template("{prefix:12} {pos} ok/skip").unwrap_or_else(|_| ProgressStyle::default_bar()),
    );
    success.set_prefix("success");
    let failed = multi.add(ProgressBar::new(0));
    failed.set_style(
        ProgressStyle::with_template("{prefix:12} {pos} failed").unwrap_or_else(|_| ProgressStyle::default_bar()),
    );
    failed.set_prefix("failed");
    (multi, overall, success, failed)
}

fn print_summary(quiet: bool, interrupted: bool, stats: BatchStats, elapsed: std::time::Duration) {
    if quiet {
        return;
    }
    if interrupted {
        eprintln!("Interrupted.");
    } else {
        eprintln!("Complete.");
    }
    eprintln!(
        "success: {}  skipped: {}  failed: {}  elapsed: {:.2}s",
        stats.success,
        stats.skipped,
        stats.failed,
        elapsed.as_secs_f64()
    );
}
