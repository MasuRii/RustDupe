//! Document text extraction and normalization.
//!
//! This module provides functionality for extracting text from various document formats:
//! - PDF documents (via pdf-extract)
//! - Word documents (via docx-rs)
//! - Plain text files (TXT, MD)
//!
//! Extracted text is normalized for robust similarity comparison.

use std::fs;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during document text extraction.
#[derive(Error, Debug)]
pub enum DocumentError {
    /// An I/O error occurred while reading the file.
    #[error("I/O error for {path}: {source}")]
    Io {
        /// Path where the error occurred
        path: std::path::PathBuf,
        /// The underlying I/O error
        #[source]
        source: std::io::Error,
    },

    /// An error occurred during PDF extraction.
    #[error("Failed to extract text from PDF {path}: {message}")]
    PdfError {
        /// Path to the PDF file
        path: std::path::PathBuf,
        /// Error message
        message: String,
    },

    /// An error occurred during DOCX extraction.
    #[error("Failed to extract text from DOCX {path}: {message}")]
    DocxError {
        /// Path to the DOCX file
        path: std::path::PathBuf,
        /// Error message
        message: String,
    },

    /// The document format is not supported.
    #[error("Unsupported document format: {0}")]
    UnsupportedFormat(String),
}

/// Extractor for document text.
pub struct DocumentExtractor;

impl DocumentExtractor {
    /// Extract text from a document at the given path.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the document file
    pub fn extract_text(path: &Path) -> Result<String, DocumentError> {
        let extension = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        match extension.as_str() {
            "pdf" => Self::extract_pdf(path),
            "docx" => Self::extract_docx(path),
            "txt" | "md" => Self::extract_plain_text(path),
            _ => Err(DocumentError::UnsupportedFormat(extension)),
        }
    }

    /// Extract text from a PDF file.
    fn extract_pdf(path: &Path) -> Result<String, DocumentError> {
        pdf_extract::extract_text(path).map_err(|e| DocumentError::PdfError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })
    }

    /// Extract text from a DOCX file.
    fn extract_docx(path: &Path) -> Result<String, DocumentError> {
        let bytes = fs::read(path).map_err(|e| DocumentError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;

        let docx = docx_rs::read_docx(&bytes).map_err(|e| DocumentError::DocxError {
            path: path.to_path_buf(),
            message: e.to_string(),
        })?;

        let mut text = String::new();
        for child in docx.document.children {
            Self::extract_text_from_child(&child, &mut text);
        }

        Ok(text)
    }

    /// Recursively extract text from DOCX document children.
    fn extract_text_from_child(child: &docx_rs::DocumentChild, text: &mut String) {
        match child {
            docx_rs::DocumentChild::Paragraph(p) => {
                for child in &p.children {
                    if let docx_rs::ParagraphChild::Run(r) = child {
                        for child in &r.children {
                            if let docx_rs::RunChild::Text(t) = child {
                                text.push_str(&t.text);
                            }
                        }
                    }
                }
                text.push('\n');
            }
            docx_rs::DocumentChild::Table(t) => {
                for row_child in &t.rows {
                    let docx_rs::TableChild::TableRow(tr) = row_child;
                    for cell_child in &tr.cells {
                        let docx_rs::TableRowChild::TableCell(tc) = cell_child;
                        for child in &tc.children {
                            match child {
                                docx_rs::TableCellContent::Paragraph(p) => {
                                    Self::extract_text_from_child(
                                        &docx_rs::DocumentChild::Paragraph(Box::new(p.clone())),
                                        text,
                                    );
                                }
                                docx_rs::TableCellContent::Table(t) => {
                                    Self::extract_text_from_child(
                                        &docx_rs::DocumentChild::Table(Box::new(t.clone())),
                                        text,
                                    );
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Extract text from a plain text file.
    fn extract_plain_text(path: &Path) -> Result<String, DocumentError> {
        fs::read_to_string(path).map_err(|e| DocumentError::Io {
            path: path.to_path_buf(),
            source: e,
        })
    }

    /// Normalize text for robust similarity comparison.
    ///
    /// Normalization includes:
    /// - Converting to lowercase
    /// - Removing punctuation
    /// - Normalizing whitespace
    pub fn normalize_text(text: &str) -> String {
        text.to_lowercase()
            .chars()
            .filter(|c| !c.is_ascii_punctuation())
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_normalize_text() {
        let input = "Hello, World! This is a TEST.   With multiple   spaces and \n newlines.";
        let expected = "hello world this is a test with multiple spaces and newlines";
        assert_eq!(DocumentExtractor::normalize_text(input), expected);
    }

    #[test]
    fn test_extract_plain_text() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "Hello world").unwrap();

        let path = file.path();
        let mut text_path = path.to_path_buf();
        text_path.set_extension("txt");
        fs::rename(path, &text_path).unwrap();

        let extracted = DocumentExtractor::extract_text(&text_path).unwrap();
        assert_eq!(extracted.trim(), "Hello world");

        fs::remove_file(text_path).unwrap();
    }

    #[test]
    fn test_unsupported_format() {
        let file = NamedTempFile::new().unwrap();
        let path = file.path();
        let mut exe_path = path.to_path_buf();
        exe_path.set_extension("exe");
        fs::rename(path, &exe_path).unwrap();

        let result = DocumentExtractor::extract_text(&exe_path);
        assert!(matches!(result, Err(DocumentError::UnsupportedFormat(_))));

        fs::remove_file(exe_path).unwrap();
    }
}
