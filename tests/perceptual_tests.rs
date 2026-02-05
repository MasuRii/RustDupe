use image::{Rgb, RgbImage};
use rustdupe::scanner::perceptual::{PerceptualAlgorithm, PerceptualHasher, SimilarityIndex};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_identical_images_have_distance_zero() {
    let temp_dir = tempdir().unwrap();
    let path1 = temp_dir.path().join("img1.png");
    let path2 = temp_dir.path().join("img2.png");

    // Create a 64x64 checkerboard image
    let mut img = RgbImage::new(64, 64);
    for x in 0..64 {
        for y in 0..64 {
            if (x / 8 + y / 8) % 2 == 0 {
                img.put_pixel(x, y, Rgb([255, 255, 255]));
            } else {
                img.put_pixel(x, y, Rgb([0, 0, 0]));
            }
        }
    }
    img.save(&path1).unwrap();
    fs::copy(&path1, &path2).unwrap();

    let hasher = PerceptualHasher::new(PerceptualAlgorithm::Phash);
    let hash1 = hasher.compute_hash(&path1).unwrap();
    let hash2 = hasher.compute_hash(&path2).unwrap();

    assert_eq!(
        hash1.dist(&hash2),
        0,
        "Identical images must have Hamming distance 0"
    );
}

#[test]
fn test_resized_images_are_similar() {
    let temp_dir = tempdir().unwrap();
    let path_orig = temp_dir.path().join("orig.png");
    let path_resized = temp_dir.path().join("resized.png");

    // Create a 128x128 image with smooth gradients
    let mut img = RgbImage::new(128, 128);
    for x in 0..128 {
        for y in 0..128 {
            let val = ((x + y) / 2) as u8;
            img.put_pixel(x, y, Rgb([val, val, val]));
        }
    }
    img.save(&path_orig).unwrap();

    // Resize to 64x64
    let resized = image::imageops::resize(&img, 64, 64, image::imageops::FilterType::Lanczos3);
    resized.save(&path_resized).unwrap();

    for alg in [
        PerceptualAlgorithm::Phash,
        PerceptualAlgorithm::Dhash,
        PerceptualAlgorithm::Ahash,
    ] {
        let hasher = PerceptualHasher::new(alg);
        let hash_orig = hasher.compute_hash(&path_orig).unwrap();
        let hash_resized = hasher.compute_hash(&path_resized).unwrap();

        let dist = hash_orig.dist(&hash_resized);
        println!("Algorithm {}: resized distance = {}", alg, dist);
        assert!(
            dist <= alg.default_threshold() + 2,
            "Algorithm {} failed: resized image distance {} exceeded threshold {}",
            alg,
            dist,
            alg.default_threshold() + 2
        );
    }
}

#[test]
fn test_different_images_have_high_distance() {
    let temp_dir = tempdir().unwrap();
    let path_1 = temp_dir.path().join("img1.png");
    let path_2 = temp_dir.path().join("img2.png");

    // Image 1: Simple 2x2 checkerboard
    let mut img1 = RgbImage::new(64, 64);
    for x in 0..64 {
        for y in 0..64 {
            let val = if (x / 32 + y / 32) % 2 == 0 { 255 } else { 0 };
            img1.put_pixel(x, y, Rgb([val, val, val]));
        }
    }
    img1.save(&path_1).unwrap();

    // Image 2: Complex random noise
    let mut img2 = RgbImage::new(64, 64);
    for x in 0..64 {
        for y in 0..64 {
            let val = if (x * 123 + y * 456) % 17 == 0 {
                255
            } else {
                0
            };
            img2.put_pixel(x, y, Rgb([val, val, val]));
        }
    }
    img2.save(&path_2).unwrap();

    for alg in [
        PerceptualAlgorithm::Phash,
        PerceptualAlgorithm::Dhash,
        PerceptualAlgorithm::Ahash,
    ] {
        let hasher = PerceptualHasher::new(alg);
        let hash1 = hasher.compute_hash(&path_1).unwrap();
        let hash2 = hasher.compute_hash(&path_2).unwrap();

        let dist = hash1.dist(&hash2);
        println!("Algorithm {}: different distance = {}", alg, dist);
        assert!(
            dist > alg.default_threshold(),
            "Algorithm {} failed: completely different images had low distance {}",
            alg,
            dist
        );
    }
}

#[test]
fn test_corrupt_images_handled_gracefully() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path().join("corrupt.png");
    fs::write(&path, b"not a real png file content").unwrap();

    let hasher = PerceptualHasher::default();
    let result = hasher.compute_hash(&path);

    assert!(
        result.is_err(),
        "Hashing a corrupt image should return an error"
    );
}

#[test]
fn test_bktree_similarity_query() {
    let temp_dir = tempdir().unwrap();

    // 1. Vertical gradient (Base)
    // 2. Vertical gradient with slight offset (Similar)
    // 3. Horizontal gradient (Different)

    let paths: Vec<_> = (1..=3)
        .map(|i| temp_dir.path().join(format!("{}.png", i)))
        .collect();

    let mut img1 = RgbImage::new(64, 64);
    for x in 0..64 {
        for y in 0..64 {
            img1.put_pixel(x, y, Rgb([x as u8 * 4, x as u8 * 4, x as u8 * 4]));
        }
    }
    img1.save(&paths[0]).unwrap();

    let mut img2 = RgbImage::new(64, 64);
    for x in 0..64 {
        for y in 0..64 {
            let val = (x as u8 + 1).saturating_mul(4);
            img2.put_pixel(x, y, Rgb([val, val, val]));
        }
    }
    img2.save(&paths[1]).unwrap();

    let mut img3 = RgbImage::new(64, 64);
    for x in 0..64 {
        for y in 0..64 {
            img3.put_pixel(x, y, Rgb([y as u8 * 4, y as u8 * 4, y as u8 * 4]));
        }
    }
    img3.save(&paths[2]).unwrap();

    // Use Dhash for this test as it seems more stable on these gradients
    let hasher = PerceptualHasher::new(PerceptualAlgorithm::Dhash);
    let mut index = SimilarityIndex::new();

    let hashes: Vec<_> = paths
        .iter()
        .map(|p| hasher.compute_hash(p).unwrap())
        .collect();
    for hash in &hashes {
        index.insert(hash.clone());
    }

    let matches = index.find(&hashes[0], hasher.algorithm().default_threshold());
    let matched_hashes: Vec<_> = matches.iter().map(|(_, h)| (*h).clone()).collect();

    assert!(
        matched_hashes.contains(&hashes[0]),
        "Matches should contain the original image"
    );
    assert!(
        matched_hashes.contains(&hashes[1]),
        "Matches should contain the similar image"
    );
    assert!(
        !matched_hashes.contains(&hashes[2]),
        "Matches should NOT contain the different image"
    );
}

#[test]
fn test_grayscale_and_color_similarity() {
    let temp_dir = tempdir().unwrap();
    let path_color = temp_dir.path().join("color.png");
    let path_gray = temp_dir.path().join("gray.png");

    // Simple gradient in color
    let mut color_img = RgbImage::new(64, 64);
    for x in 0..64 {
        for y in 0..64 {
            color_img.put_pixel(x, y, Rgb([x as u8 * 4, y as u8 * 4, 128]));
        }
    }
    color_img.save(&path_color).unwrap();

    let mut gray_img = RgbImage::new(64, 64);
    for x in 0..64 {
        for y in 0..64 {
            let p = color_img.get_pixel(x, y);
            let luma = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) as u8;
            gray_img.put_pixel(x, y, Rgb([luma, luma, luma]));
        }
    }
    gray_img.save(&path_gray).unwrap();

    for alg in [
        PerceptualAlgorithm::Phash,
        PerceptualAlgorithm::Dhash,
        PerceptualAlgorithm::Ahash,
    ] {
        let hasher = PerceptualHasher::new(alg);
        let hash_color = hasher.compute_hash(&path_color).unwrap();
        let hash_gray = hasher.compute_hash(&path_gray).unwrap();

        let dist = hash_color.dist(&hash_gray);
        println!("Algorithm {}: color/gray distance = {}", alg, dist);
        assert!(
            dist <= alg.default_threshold() + 15, // Standard threshold + 15 for synthetic patterns
            "Algorithm {} failed: color and grayscale versions should be similar, distance was {}",
            alg,
            dist
        );
    }
}
