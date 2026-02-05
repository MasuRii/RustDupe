//! Perceptual image hashing for similarity detection.
//!
//! This module provides the `PerceptualHasher` which can compute hashes
//! for images that remain stable under common transformations like
//! resizing, rotation (slight), and compression.

use image_hasher::{HashAlg, HasherConfig, ImageHash};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Supported perceptual hashing algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PerceptualAlgorithm {
    /// pHash (Perceptual Hash) - DCT-based, most resilient to transformations.
    #[default]
    Phash,
    /// dHash (Difference Hash) - Gradient-based, very fast and effective.
    Dhash,
    /// aHash (Average Hash) - Mean-based, fast but less resilient.
    Ahash,
}

impl std::fmt::Display for PerceptualAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Phash => write!(f, "pHash"),
            Self::Dhash => write!(f, "dHash"),
            Self::Ahash => write!(f, "aHash"),
        }
    }
}

/// Errors that can occur during perceptual hashing.
#[derive(Debug, Error)]
pub enum PerceptualError {
    /// Failed to open or decode the image.
    #[error("Failed to load image {0}: {1}")]
    LoadError(String, #[source] image::ImageError),

    /// Image format not supported for hashing.
    #[error("Unsupported image format: {0}")]
    UnsupportedFormat(String),
}

/// Computes perceptual hashes for images.
pub struct PerceptualHasher {
    hasher: image_hasher::Hasher,
    algorithm: PerceptualAlgorithm,
}

impl PerceptualHasher {
    /// Create a new `PerceptualHasher` with the given algorithm.
    pub fn new(algorithm: PerceptualAlgorithm) -> Self {
        let mut config = HasherConfig::new();

        match algorithm {
            PerceptualAlgorithm::Phash => {
                config = config.hash_alg(HashAlg::Median).preproc_dct();
            }
            PerceptualAlgorithm::Dhash => {
                config = config.hash_alg(HashAlg::Gradient);
            }
            PerceptualAlgorithm::Ahash => {
                config = config.hash_alg(HashAlg::Mean);
            }
        }

        Self {
            hasher: config.to_hasher(),
            algorithm,
        }
    }

    /// Compute the perceptual hash for an image at the given path.
    pub fn compute_hash<P: AsRef<Path>>(&self, path: P) -> Result<ImageHash, PerceptualError> {
        let path = path.as_ref();
        let img = image::open(path)
            .map_err(|e| PerceptualError::LoadError(path.display().to_string(), e))?;

        Ok(self.hasher.hash_image(&img))
    }

    /// Get the algorithm used by this hasher.
    pub fn algorithm(&self) -> PerceptualAlgorithm {
        self.algorithm
    }
}

impl Default for PerceptualHasher {
    fn default() -> Self {
        Self::new(PerceptualAlgorithm::Phash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_perceptual_algorithms_display() {
        assert_eq!(PerceptualAlgorithm::Phash.to_string(), "pHash");
        assert_eq!(PerceptualAlgorithm::Dhash.to_string(), "dHash");
        assert_eq!(PerceptualAlgorithm::Ahash.to_string(), "aHash");
    }

    #[test]
    fn test_perceptual_hasher_new() {
        let hasher = PerceptualHasher::new(PerceptualAlgorithm::Phash);
        assert_eq!(hasher.algorithm(), PerceptualAlgorithm::Phash);

        let hasher = PerceptualHasher::new(PerceptualAlgorithm::Dhash);
        assert_eq!(hasher.algorithm(), PerceptualAlgorithm::Dhash);

        let hasher = PerceptualHasher::new(PerceptualAlgorithm::Ahash);
        assert_eq!(hasher.algorithm(), PerceptualAlgorithm::Ahash);
    }

    #[test]
    fn test_invalid_image() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("invalid.txt");
        let mut file = File::create(&file_path).unwrap();
        writeln!(file, "not an image").unwrap();

        let hasher = PerceptualHasher::default();
        let result = hasher.compute_hash(&file_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_compute_hash_real_image() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_image.png");

        // Create a 10x10 RGB image
        let img = image::RgbImage::new(10, 10);
        img.save(&file_path).unwrap();

        let hasher = PerceptualHasher::new(PerceptualAlgorithm::Phash);
        let hash = hasher.compute_hash(&file_path).unwrap();

        // A 10x10 black image should have a stable hash
        assert!(hash.as_bytes().len() > 0);
    }
}
