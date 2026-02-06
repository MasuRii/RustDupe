use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use tempfile::tempdir;

#[test]
fn test_perceptual_hashing_integration() {
    let dir = tempdir().unwrap();

    // Create a real image (10x10 PNG)
    let img_path = dir.path().join("image.png");
    let img = image::RgbImage::new(10, 10);
    img.save(&img_path).unwrap();

    // Create a duplicate image
    let img_dup_path = dir.path().join("image_dup.png");
    std::fs::copy(&img_path, &img_dup_path).unwrap();

    // Create a non-image file
    let txt_path = dir.path().join("test.txt");
    std::fs::write(&txt_path, "not an image").unwrap();

    let config = FinderConfig::default().with_similar_images(true);
    let finder = DuplicateFinder::new(config);

    let (groups, summary) = finder.find_duplicates(dir.path()).unwrap();

    // Should find the exact duplicate pair.
    // The similar image group is redundant and should be filtered out.
    assert_eq!(groups.len(), 1);
    assert!(!groups[0].is_similar);

    // Should have processed 2 images for perceptual hashing
    assert_eq!(summary.images_perceptual_hashed, 2);

    // Check that perceptual hashes are present
    for group in groups {
        for file in group.files {
            if file.is_image() {
                assert!(file.perceptual_hash.is_some());
            } else {
                assert!(file.perceptual_hash.is_none());
            }
        }
    }
}

#[test]
fn test_perceptual_hashing_cache() {
    let dir = tempdir().unwrap();
    let cache_dir = tempdir().unwrap();
    let cache_path = cache_dir.path().join("cache.db");

    let img_path = dir.path().join("image.png");
    let img = image::RgbImage::new(10, 10);
    img.save(&img_path).unwrap();

    let config = FinderConfig::default()
        .with_similar_images(true)
        .with_cache(std::sync::Arc::new(
            rustdupe::cache::HashCache::new(&cache_path).unwrap(),
        ));

    let finder = DuplicateFinder::new(config.clone());
    let (_, summary) = finder.find_duplicates(dir.path()).unwrap();
    assert_eq!(summary.images_perceptual_hashed, 1);
    assert_eq!(summary.images_perceptual_hash_cache_hits, 0);

    // Second run should use cache
    let finder2 = DuplicateFinder::new(config);
    let (_, summary2) = finder2.find_duplicates(dir.path()).unwrap();
    assert_eq!(summary2.images_perceptual_hashed, 1);
    assert_eq!(summary2.images_perceptual_hash_cache_hits, 1);
}

#[test]
fn test_perceptual_hashing_disabled() {
    let dir = tempdir().unwrap();
    let img_path = dir.path().join("image.png");
    let img = image::RgbImage::new(10, 10);
    img.save(&img_path).unwrap();

    let config = FinderConfig::default().with_similar_images(false);
    let finder = DuplicateFinder::new(config);

    let (_, summary) = finder.find_duplicates(dir.path()).unwrap();

    assert_eq!(summary.images_perceptual_hashed, 0);
}
