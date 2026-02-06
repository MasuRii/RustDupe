use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use rustdupe::scanner::document::{DocumentError, DocumentExtractor, SimHasher};
use std::fs;
use tempfile::tempdir;

#[test]
fn test_simhash_identical_distance_zero() {
    let text1 = "The quick brown fox jumps over the lazy dog. It was a sunny day in the park.";
    let text2 = "The quick brown fox jumps over the lazy dog. It was a sunny day in the park.";

    let hash1 = SimHasher::compute_fingerprint(text1);
    let hash2 = SimHasher::compute_fingerprint(text2);

    assert_eq!(SimHasher::hamming_distance(hash1, hash2), 0);
}

#[test]
fn test_simhash_similar_low_distance() {
    let text1 = "The quick brown fox jumps over the lazy dog. It was a sunny day in the park.";
    let text2 = "The quick brown fox jumps over the active dog. It was a sunny day in the park.";

    let hash1 = SimHasher::compute_fingerprint(text1);
    let hash2 = SimHasher::compute_fingerprint(text2);

    let distance = SimHasher::hamming_distance(hash1, hash2);
    println!("Similar distance: {}", distance);

    // Similarity threshold is usually around 3-5 for 64-bit SimHash.
    // Given the small change, it should be low.
    assert!(distance > 0);
    assert!(distance <= 15);
}

#[test]
fn test_simhash_different_high_distance() {
    let text1 = "The quick brown fox jumps over the lazy dog. It was a sunny day in the park.";
    let text2 = "Rust is a systems programming language that provides memory safety without garbage collection.";

    let hash1 = SimHasher::compute_fingerprint(text1);
    let hash2 = SimHasher::compute_fingerprint(text2);

    let distance = SimHasher::hamming_distance(hash1, hash2);
    println!("Different distance: {}", distance);

    assert!(distance > 15);
}

#[test]
fn test_document_similarity_matching() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path();

    // Create two similar documents
    let doc1_path = path.join("doc1.txt");
    let doc2_path = path.join("doc2.txt");
    let doc3_path = path.join("doc3.txt"); // different

    fs::write(
        &doc1_path,
        "The quick brown fox jumps over the lazy dog. It was a sunny day in the park.",
    )
    .unwrap();
    fs::write(
        &doc2_path,
        "The quick brown fox jumps over the active dog. It was a sunny day in the park.",
    )
    .unwrap();
    fs::write(&doc3_path, "Rust is a systems programming language that provides memory safety without garbage collection.").unwrap();

    let config = FinderConfig::default()
        .with_similar_documents(true)
        .with_doc_similarity_threshold(Some(15));

    let finder = DuplicateFinder::new(config);
    let (groups, summary) = finder.find_duplicates(path).unwrap();

    // doc1 and doc2 should be in a similar group
    assert!(groups.len() >= 1);
    let similar_group = groups
        .iter()
        .find(|g| g.is_similar)
        .expect("Should find a similar group");

    assert_eq!(similar_group.files.len(), 2);
    let paths: Vec<_> = similar_group
        .files
        .iter()
        .map(|f| f.path.file_name().unwrap().to_str().unwrap())
        .collect();
    assert!(paths.contains(&"doc1.txt"));
    assert!(paths.contains(&"doc2.txt"));
    assert!(!paths.contains(&"doc3.txt"));

    assert_eq!(summary.documents_fingerprinted, 3);
}

#[test]
fn test_document_identical_is_not_double_counted() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path();

    // Create two identical documents
    let doc1_path = path.join("doc1.txt");
    let doc2_path = path.join("doc2.txt");

    let content = "The quick brown fox jumps over the lazy dog.";
    fs::write(&doc1_path, content).unwrap();
    fs::write(&doc2_path, content).unwrap();

    let config = FinderConfig::default().with_similar_documents(true);

    let finder = DuplicateFinder::new(config);
    let (groups, summary) = finder.find_duplicates(path).unwrap();

    // They should be matched as exact duplicates first
    assert_eq!(groups.len(), 1);
    assert!(!groups[0].is_similar); // Should be exact match

    // ScanSummary should reflect this
    assert_eq!(summary.duplicate_groups, 1);
}

#[test]
fn test_corrupt_documents_handled_gracefully() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path();

    // Create a "corrupt" PDF (just random bytes)
    let corrupt_pdf_path = path.join("corrupt.pdf");
    fs::write(&corrupt_pdf_path, b"NOT A PDF %PDF-1.4 garbage content").unwrap();

    // Create a "corrupt" DOCX
    let corrupt_docx_path = path.join("corrupt.docx");
    fs::write(&corrupt_docx_path, b"NOT A DOCX zip header missing").unwrap();

    let config = FinderConfig::default().with_similar_documents(true);
    let finder = DuplicateFinder::new(config);

    // Scan should complete without error
    let (_groups, summary) = finder.find_duplicates(path).unwrap();

    // Summary should show 2 files processed
    assert_eq!(summary.total_files, 2);

    // Extraction should fail gracefully
    let pdf_result = DocumentExtractor::extract_text(&corrupt_pdf_path);
    assert!(pdf_result.is_err());
    assert!(matches!(
        pdf_result.unwrap_err(),
        DocumentError::PdfError { .. }
    ));

    let docx_result = DocumentExtractor::extract_text(&corrupt_docx_path);
    assert!(docx_result.is_err());
    assert!(matches!(
        docx_result.unwrap_err(),
        DocumentError::DocxError { .. }
    ));
}

#[test]
fn test_unsupported_format_skipped() {
    let temp_dir = tempdir().unwrap();
    let path = temp_dir.path();

    let exe_path = path.join("program.exe");
    fs::write(&exe_path, b"binary content").unwrap();

    let config = FinderConfig::default().with_similar_documents(true);
    let finder = DuplicateFinder::new(config);

    let (_groups, summary) = finder.find_duplicates(path).unwrap();

    // Only documents are processed for SimHash
    assert_eq!(summary.documents_fingerprinted, 0);

    let result = DocumentExtractor::extract_text(&exe_path);
    assert!(matches!(result, Err(DocumentError::UnsupportedFormat(_))));
}
