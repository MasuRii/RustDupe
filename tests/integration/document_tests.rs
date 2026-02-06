use rustdupe::duplicates::{DuplicateFinder, FinderConfig};
use std::fs;
use tempfile::tempdir;

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
        .with_doc_similarity_threshold(Some(20)); // even more generous

    let finder = DuplicateFinder::new(config);
    let (groups, summary) = finder.find_duplicates(path).unwrap();

    println!("Summary: {:?}", summary);
    println!("Groups: {:?}", groups);

    for file in fs::read_dir(path).unwrap() {
        let p = file.unwrap().path();
        let text = fs::read_to_string(&p).unwrap();
        let fp = rustdupe::scanner::document::SimHasher::compute_fingerprint(&text);
        println!("{}: {}", p.display(), fp);
    }

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
    // Phase 3 fullhash should find them.
    assert_eq!(groups.len(), 1);
    assert!(!groups[0].is_similar); // Should be exact match

    // ScanSummary should reflect this
    assert_eq!(summary.duplicate_groups, 1);
}
