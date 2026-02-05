use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use rustdupe::scanner::{Hasher, PerceptualAlgorithm, PerceptualHasher, Walker, WalkerConfig};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tempfile::TempDir;

// Helper to create a test directory with a specific structure
fn setup_test_dir(depth: usize, files_per_dir: usize) -> TempDir {
    let temp_dir = TempDir::new().unwrap();
    create_dir_recursive(temp_dir.path().to_path_buf(), depth, files_per_dir);
    temp_dir
}

fn create_dir_recursive(path: PathBuf, depth: usize, files_per_dir: usize) {
    if depth == 0 {
        return;
    }

    if !path.exists() {
        fs::create_dir_all(&path).expect("Failed to create dir");
    }

    for i in 0..files_per_dir {
        let file_path = path.join(format!("file_{}.txt", i));
        fs::write(file_path, "some content to make it a real file").expect("Failed to write file");
    }

    if depth > 1 {
        for i in 0..2 {
            // 2 subdirectories per level
            let sub_dir = path.join(format!("dir_{}", i));
            create_dir_recursive(sub_dir, depth - 1, files_per_dir);
        }
    }
}

// 1. Directory Walking Benchmarks
fn bench_walker(c: &mut Criterion) {
    let temp_dir = setup_test_dir(4, 10); // depth 4, 10 files per dir -> roughly 150 files
    let config = WalkerConfig::default();

    c.bench_function("walker_150_files", |b| {
        b.iter(|| {
            let walker = Walker::new(temp_dir.path(), config.clone());
            let files: Vec<_> = walker.walk().collect();
            black_box(files);
        })
    });
}

// 2. Hashing Benchmarks
fn bench_hasher(c: &mut Criterion) {
    let mut group = c.benchmark_group("hasher");
    let hasher = Hasher::new();

    for size_kb in [1, 1024, 10240] {
        // 1KB, 1MB, 10MB
        let data = vec![b'a'; size_kb * 1024];
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("bench_file.dat");
        fs::write(&file_path, &data).expect("Failed to write bench file");

        group.bench_with_input(format!("blake3_{}KB", size_kb), &file_path, |b, path| {
            b.iter(|| {
                let hash = hasher.full_hash(path).unwrap();
                black_box(hash);
            });
        });
    }
    group.finish();
}

// 3. Perceptual Hashing Benchmarks
fn bench_perceptual(c: &mut Criterion) {
    let mut group = c.benchmark_group("perceptual_hasher");

    // Create a 256x256 image (gradient)
    let mut img = image::RgbImage::new(256, 256);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        *pixel = image::Rgb([x as u8, y as u8, 128u8]);
    }

    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("bench_img.png");
    img.save(&file_path).expect("Failed to save bench image");

    for alg in [
        PerceptualAlgorithm::Phash,
        PerceptualAlgorithm::Dhash,
        PerceptualAlgorithm::Ahash,
    ] {
        let hasher = PerceptualHasher::new(alg);
        group.bench_with_input(format!("{:?}", alg), &file_path, |b, path| {
            b.iter(|| {
                let hash = hasher.compute_hash(path).unwrap();
                black_box(hash);
            });
        });
    }
    group.finish();
}

// 4. Full Pipeline Benchmark
fn bench_pipeline(c: &mut Criterion) {
    let temp_dir = setup_test_dir(3, 10); // ~70 files
                                          // Create some duplicates
    let src = temp_dir.path().join("file_0.txt");
    if src.exists() {
        for i in 1..10 {
            let dst = temp_dir.path().join(format!("dup_{}.txt", i));
            fs::copy(&src, &dst).expect("Failed to copy duplicate");
        }
    }

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let config = FinderConfig::default()
        .with_walker_config(WalkerConfig::default())
        .with_shutdown_flag(shutdown_flag);

    let finder = DuplicateFinder::new(config);

    c.bench_function("pipeline_approx_80_files", |b| {
        b.iter(|| {
            let results = finder
                .find_duplicates_in_paths(vec![temp_dir.path().to_path_buf()])
                .unwrap();
            black_box(results);
        })
    });
}

criterion_group!(
    benches,
    bench_walker,
    bench_hasher,
    bench_perceptual,
    bench_pipeline
);
criterion_main!(benches);
