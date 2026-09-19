//! Aspect-preserving fit calculations for resize targets.

/// Default maximum edge length in pixels.
pub const DEFAULT_MAX_SIZE: u32 = 4096;

/// Computes destination dimensions that fit inside `max_size` without upscaling.
///
/// The longer edge is limited to `max_size`. Images that already fit are unchanged.
///
/// # Arguments
///
/// * `width` - Source width in pixels
/// * `height` - Source height in pixels
/// * `max_size` - Maximum allowed edge length
///
/// # Returns
///
/// The `(width, height)` pair after fitting.
///
/// # Examples
///
/// ```
/// use pictures4096::geometry::fit_dimensions;
///
/// assert_eq!(fit_dimensions(8000, 4000, 4096), (4096, 2048));
/// assert_eq!(fit_dimensions(1024, 768, 4096), (1024, 768));
/// ```
#[must_use]
pub fn fit_dimensions(width: u32, height: u32, max_size: u32) -> (u32, u32) {
    if width == 0 || height == 0 || max_size == 0 {
        return (width.max(1).min(max_size.max(1)), height.max(1).min(max_size.max(1)));
    }
    if width <= max_size && height <= max_size {
        return (width, height);
    }

    let width_f = f64::from(width);
    let height_f = f64::from(height);
    let max_f = f64::from(max_size);
    let scale = (max_f / width_f).min(max_f / height_f);
    // Rounded values are clamped to `[1, max_size]`, so the u32 cast cannot truncate or go negative.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let new_width = (width_f * scale).round().clamp(1.0, max_f) as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let new_height = (height_f * scale).round().clamp(1.0, max_f) as u32;
    (new_width, new_height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_already_fitting_images() {
        assert_eq!(fit_dimensions(100, 200, 4096), (100, 200));
    }

    #[test]
    fn scales_landscape_to_max_width() {
        assert_eq!(fit_dimensions(8192, 4096, 4096), (4096, 2048));
    }

    #[test]
    fn scales_portrait_to_max_height() {
        assert_eq!(fit_dimensions(2000, 8000, 4096), (1024, 4096));
    }

    #[test]
    fn handles_zero_max_size() {
        let (width, height) = fit_dimensions(10, 10, 0);
        assert!(width >= 1);
        assert!(height >= 1);
    }
}
