//! Perceptual image hashing for similarity detection.
//!
//! This module provides the `PerceptualHasher` which can compute hashes
//! for images that remain stable under common transformations like
//! resizing, rotation (slight), and compression.

use bk_tree::{BKTree, Metric};
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

impl PerceptualAlgorithm {
    /// Get the default similarity threshold (Hamming distance) for this algorithm.
    pub fn default_threshold(&self) -> u32 {
        match self {
            Self::Phash => 10,
            Self::Dhash => 2,
            Self::Ahash => 5,
        }
    }
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

/// Metric for comparing `ImageHash` values using Hamming distance.
#[derive(Default, Clone, Copy, Debug)]
pub struct ImageHashMetric;

impl Metric<ImageHash> for ImageHashMetric {
    fn distance(&self, a: &ImageHash, b: &ImageHash) -> u32 {
        a.dist(b)
    }

    fn threshold_distance(&self, a: &ImageHash, b: &ImageHash, threshold: u32) -> Option<u32> {
        let d = self.distance(a, b);
        if d <= threshold {
            Some(d)
        } else {
            None
        }
    }
}

/// A similarity index for perceptual hashes using a BK-tree.
///
/// Enables efficient similarity search with O(log n) complexity.
pub struct SimilarityIndex {
    tree: BKTree<ImageHash, ImageHashMetric>,
    count: usize,
}

impl SimilarityIndex {
    /// Create a new empty similarity index.
    pub fn new() -> Self {
        Self {
            tree: BKTree::new(ImageHashMetric),
            count: 0,
        }
    }

    /// Add an image hash to the index.
    pub fn insert(&mut self, hash: ImageHash) {
        self.tree.add(hash);
        self.count += 1;
    }

    /// Find all hashes in the index within the given Hamming distance.
    ///
    /// Returns a list of (distance, hash) pairs.
    pub fn find(&self, hash: &ImageHash, max_distance: u32) -> Vec<(u32, &ImageHash)> {
        self.tree.find(hash, max_distance).collect()
    }

    /// Returns the number of items in the index.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Returns true if the index is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
}

impl Default for SimilarityIndex {
    fn default() -> Self {
        Self::new()
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
    fn test_perceptual_algorithm_thresholds() {
        assert_eq!(PerceptualAlgorithm::Phash.default_threshold(), 10);
        assert_eq!(PerceptualAlgorithm::Dhash.default_threshold(), 2);
        assert_eq!(PerceptualAlgorithm::Ahash.default_threshold(), 5);
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
        assert!(!hash.as_bytes().is_empty());
    }

    #[test]
    fn test_similarity_index_basic() {
        let mut index = SimilarityIndex::new();
        assert!(index.is_empty());

        // Create some dummy hashes
        let h1 = ImageHash::from_bytes(&[0, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        let h2 = ImageHash::from_bytes(&[0, 0, 0, 0, 0, 0, 0, 1]).unwrap(); // distance 1
        let h3 = ImageHash::from_bytes(&[1, 1, 1, 1, 1, 1, 1, 1]).unwrap(); // distance 8

        index.insert(h1.clone());
        index.insert(h2.clone());
        index.insert(h3.clone());

        assert_eq!(index.len(), 3);

        // Find matches for h1 with distance 1
        let matches = index.find(&h1, 1);
        assert_eq!(matches.len(), 2);
        assert!(matches.iter().any(|(d, h)| *d == 0 && **h == h1));
        assert!(matches.iter().any(|(d, h)| *d == 1 && **h == h2));

        // Find matches for h1 with distance 10
        let matches = index.find(&h1, 10);
        assert_eq!(matches.len(), 3);
    }
}
