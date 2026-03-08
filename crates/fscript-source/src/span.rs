use miette::SourceSpan;
use serde::{Deserialize, Serialize};

/// Stable identifier for a source file in a compilation session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FileId(u32);

impl FileId {
    /// Creates a new file identifier.
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    /// Returns the underlying numeric identifier.
    #[must_use]
    pub const fn as_u32(self) -> u32 {
        self.0
    }
}

/// Half-open byte span into a source file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    start: usize,
    end: usize,
}

impl Span {
    /// Creates a new span.
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// Returns the start byte offset.
    #[must_use]
    pub const fn start(self) -> usize {
        self.start
    }

    /// Returns the end byte offset.
    #[must_use]
    pub const fn end(self) -> usize {
        self.end
    }

    /// Returns the smallest span that covers both spans.
    #[must_use]
    pub const fn cover(self, other: Self) -> Self {
        Self {
            start: if self.start < other.start {
                self.start
            } else {
                other.start
            },
            end: if self.end > other.end {
                self.end
            } else {
                other.end
            },
        }
    }

    /// Returns the span length in bytes.
    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    /// Returns whether the span is empty.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start >= self.end
    }

    /// Slices a source string using the span.
    #[must_use]
    pub fn slice(self, source: &str) -> &str {
        &source[self.start..self.end]
    }
}

impl From<Span> for SourceSpan {
    fn from(value: Span) -> Self {
        Self::new(value.start.into(), value.len())
    }
}

#[cfg(test)]
mod tests {
    use miette::SourceSpan;

    use super::{FileId, Span};

    #[test]
    fn slice_returns_the_matching_substring() {
        let source = "hello world";
        let span = Span::new(6, 11);

        assert_eq!(span.slice(source), "world");
    }

    #[test]
    fn empty_spans_report_as_empty() {
        let span = Span::new(2, 2);

        assert!(span.is_empty());
        assert_eq!(span.len(), 0);
    }

    #[test]
    fn file_ids_round_trip_their_numeric_value() {
        let file_id = FileId::new(42);

        assert_eq!(file_id.as_u32(), 42);
    }

    #[test]
    fn spans_report_start_end_and_cover_ranges() {
        let left = Span::new(3, 7);
        let right = Span::new(1, 5);
        let covered = left.cover(right);
        let reversed = right.cover(left);

        assert_eq!(left.start(), 3);
        assert_eq!(left.end(), 7);
        assert_eq!(covered, Span::new(1, 7));
        assert_eq!(reversed, Span::new(1, 7));
    }

    #[test]
    fn spans_convert_into_miette_source_spans() {
        let source_span: SourceSpan = Span::new(4, 9).into();

        assert_eq!(source_span, SourceSpan::new(4.into(), 5));
    }
}
