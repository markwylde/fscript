use std::{fs, io};

use camino::{Utf8Path, Utf8PathBuf};
use miette::NamedSource;
use thiserror::Error;

/// Source text plus path and line offset metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFile {
    path: Utf8PathBuf,
    contents: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    /// Creates a source file from an in-memory string.
    #[must_use]
    pub fn new(path: Utf8PathBuf, contents: String) -> Self {
        Self {
            path,
            line_starts: compute_line_starts(&contents),
            contents,
        }
    }

    /// Loads a UTF-8 source file from disk.
    pub fn load(path: &Utf8Path) -> Result<Self, SourceLoadError> {
        let contents = fs::read_to_string(path).map_err(|source| SourceLoadError::Read {
            path: path.to_owned(),
            source,
        })?;

        Ok(Self::new(path.to_owned(), contents))
    }

    /// Returns the filesystem path.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// Returns the source contents.
    #[must_use]
    pub fn contents(&self) -> &str {
        &self.contents
    }

    /// Returns a miette-compatible named source clone for diagnostics.
    #[must_use]
    pub fn named_source(&self) -> NamedSource<String> {
        NamedSource::new(self.path.as_str(), self.contents.clone())
    }

    /// Converts a byte offset into one-based line and column numbers.
    #[must_use]
    pub fn line_column(&self, offset: usize) -> (usize, usize) {
        let bounded_offset = offset.min(self.contents.len());
        let line_index = self
            .line_starts
            .partition_point(|line_start| *line_start <= bounded_offset)
            .saturating_sub(1);
        let column = bounded_offset - self.line_starts[line_index] + 1;

        (line_index + 1, column)
    }

    /// Returns the text for a one-based line number, without its trailing newline.
    #[must_use]
    pub fn line_text(&self, line_number: usize) -> &str {
        let Some(line_index) = line_number.checked_sub(1) else {
            return "";
        };
        if line_index >= self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[line_index];
        let end = self
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(self.contents.len());
        self.contents[start..end].trim_end_matches('\n')
    }

    /// Returns the number of lines in the source file.
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

/// Loading failures for source files.
#[derive(Debug, Error)]
pub enum SourceLoadError {
    #[error("failed to read source file `{path}`")]
    Read {
        path: Utf8PathBuf,
        #[source]
        source: io::Error,
    },
}

fn compute_line_starts(contents: &str) -> Vec<usize> {
    let mut starts = vec![0];

    for (index, byte) in contents.bytes().enumerate() {
        if byte == b'\n' {
            starts.push(index + 1);
        }
    }

    starts
}

#[cfg(test)]
mod tests {
    use std::{fs, io};

    use camino::Utf8PathBuf;
    use miette::SourceCode;

    use super::{SourceFile, SourceLoadError};
    use crate::span::Span;

    #[test]
    fn load_reads_utf8_files_from_disk() {
        let path = std::env::temp_dir().join("fscript-source-load.fs");
        fs::write(&path, "alpha\nbeta").expect("temp source should be writable");
        let path = Utf8PathBuf::from_path_buf(path).expect("temp paths should be utf-8");

        let source = SourceFile::load(&path).expect("temp source should load");

        assert_eq!(source.path(), path.as_path());
        assert_eq!(source.contents(), "alpha\nbeta");
        assert_eq!(source.line_count(), 2);
    }

    #[test]
    fn load_reports_read_failures_with_the_requested_path() {
        let path = Utf8PathBuf::from("missing-source.fs");

        let error = SourceFile::load(&path).expect_err("missing files should fail to load");

        match error {
            SourceLoadError::Read {
                path: error_path,
                source,
            } => {
                assert_eq!(error_path, path);
                assert_eq!(source.kind(), io::ErrorKind::NotFound);
            }
        }
    }

    #[test]
    fn line_column_tracks_offsets_across_multiple_lines() {
        let source = SourceFile::new(Utf8PathBuf::from("example.fs"), "alpha\nbeta".to_owned());

        assert_eq!(source.line_column(0), (1, 1));
        assert_eq!(source.line_column(4), (1, 5));
        assert_eq!(source.line_column(6), (2, 1));
        assert_eq!(source.line_column(9), (2, 4));
    }

    #[test]
    fn line_column_bounds_offsets_past_the_end() {
        let source = SourceFile::new(Utf8PathBuf::from("example.fs"), "abc".to_owned());

        assert_eq!(source.line_column(99), (1, 4));
    }

    #[test]
    fn line_text_returns_a_single_line_without_the_newline() {
        let source = SourceFile::new(Utf8PathBuf::from("example.fs"), "alpha\nbeta\n".to_owned());

        assert_eq!(source.line_text(1), "alpha");
        assert_eq!(source.line_text(2), "beta");
    }

    #[test]
    fn line_text_returns_empty_for_out_of_range_lines() {
        let source = SourceFile::new(Utf8PathBuf::from("example.fs"), "alpha\nbeta".to_owned());

        assert_eq!(source.line_text(0), "");
        assert_eq!(source.line_text(3), "");
    }

    #[test]
    fn named_source_keeps_the_original_name_and_contents() {
        let source = SourceFile::new(Utf8PathBuf::from("example.fs"), "alpha".to_owned());
        let named = source.named_source();

        assert_eq!(named.name(), "example.fs");
        assert_eq!(
            named
                .inner()
                .read_span(&Span::new(0, 5).into(), 0, 0)
                .unwrap()
                .data(),
            b"alpha"
        );
    }
}
