//! Shared source loading, spans, and line mapping for the FScript toolchain.

mod source_file;
mod span;

pub use source_file::{SourceFile, SourceLoadError};
pub use span::{FileId, Span};
