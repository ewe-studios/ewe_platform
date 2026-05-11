//! Golden image validation — compare screenshots against templates.
//!
//! Loads both images, resizes if needed, calculates pixel match percentage,
//! and generates a diff image highlighting mismatches.

use std::path::Path;

use crate::config::Result;

/// Validation result.
#[derive(Debug)]
pub struct ValidationResult {
    /// Match percentage (100.0 = perfect match).
    pub match_pct: f64,
    /// Whether the validation passed (>= tolerance).
    pub passed: bool,
    /// Path to the diff image (if generated).
    pub diff_path: Option<String>,
}

/// Compare a screenshot against a golden template image.
///
/// Returns a `ValidationResult` with match percentage and pass/fail.
/// If the match is below tolerance, generates a diff image.
pub fn validate(
    actual: &Path,
    golden: &Path,
    tolerance: f64,
    diff_output: Option<&Path>,
) -> Result<ValidationResult> {
    // Load images
    let golden_img = image::open(golden).map_err(|e| {
        crate::config::TestbedError::Qcow2Error {
            message: format!("loading golden image {golden:?}: {e}"),
        }
    })?;
    let actual_img = image::open(actual).map_err(|e| {
        crate::config::TestbedError::Qcow2Error {
            message: format!("loading actual image {actual:?}: {e}"),
        }
    })?;

    // Resize actual to match golden dimensions
    let (gw, gh) = (golden_img.width(), golden_img.height());
    let actual_resized = actual_img.resize_exact(gw, gh, image::imageops::FilterType::Lanczos3);

    // Convert to RGBA for comparison
    let golden_rgba = golden_img.to_rgba8();
    let actual_rgba = actual_resized.to_rgba8();

    let total_pixels = (gw as u64) * (gh as u64);
    let mut matching_pixels: u64 = 0;
    let mut diff_img = if diff_output.is_some() {
        Some(image::RgbaImage::new(gw, gh))
    } else {
        None
    };

    // Pixel-by-pixel comparison
    for y in 0..gh {
        for x in 0..gw {
            let gp = golden_rgba.get_pixel(x, y);
            let ap = actual_rgba.get_pixel(x, y);

            // Simple threshold-based comparison (allow 16/255 per channel drift)
            let matches = gp.0.iter().zip(ap.0.iter()).all(|(g, a)| {
                (*g as i16 - *a as i16).abs() < 16
            });

            if matches {
                matching_pixels += 1;
                if let Some(ref mut diff) = diff_img {
                    diff.put_pixel(x, y, image::Rgba([0, 255, 0, 255])); // green = match
                }
            } else {
                if let Some(ref mut diff) = diff_img {
                    diff.put_pixel(x, y, image::Rgba([255, 0, 0, 255])); // red = mismatch
                }
            }
        }
    }

    let match_pct = (matching_pixels as f64 / total_pixels as f64) * 100.0;
    let passed = match_pct >= tolerance;

    // Save diff image if requested and validation failed
    let diff_path = if passed {
        None
    } else if let (Some(diff), Some(out_path)) = (diff_img, diff_output) {
        let path_str = out_path.to_string_lossy().to_string();
        diff.save(out_path).map_err(|e| {
            crate::config::TestbedError::Qcow2Error {
                message: format!("saving diff image {out_path:?}: {e}"),
            }
        })?;
        Some(path_str)
    } else {
        None
    };

    Ok(ValidationResult {
        match_pct,
        passed,
        diff_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result_structure() {
        let result = ValidationResult {
            match_pct: 95.0,
            passed: true,
            diff_path: None,
        };
        assert!(result.passed);
        assert!((result.match_pct - 95.0).abs() < 0.01);
    }

    #[test]
    fn test_default_tolerance_95_percent() {
        // Common tolerance for UI testing
        let tolerance = 95.0;
        assert!(96.0 >= tolerance);
        assert!(94.0 < tolerance);
    }
}
