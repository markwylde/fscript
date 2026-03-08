//! Driver orchestration for CLI-facing compiler entrypoints.
#![allow(dead_code)]

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    rc::Rc,
};

use camino::{Utf8Path, Utf8PathBuf};
use fscript_ast::{
    BinaryOperator, BlockItem, Expr, ImportClause, ImportDecl, MatchArm, Module, ModuleItem,
    Parameter, Pattern, UnaryOperator,
};
use fscript_codegen_cranelift::CompileError;
use fscript_effects::EffectError;
use fscript_hir as hir;
use fscript_lexer::{LexDiagnostic, LexDiagnosticKind, lex};
use fscript_lower::LowerError;
use fscript_parser::{ParseDiagnostic, ParseDiagnosticKind, parse_module};
use fscript_runtime as shared_runtime;
use fscript_source::{SourceFile, SourceLoadError};
use fscript_types::TypeError;
use miette::{Diagnostic, LabeledSpan, NamedSource, SourceCode, SourceSpan};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Summary returned by a successful `fscript check`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckSummary {
    /// Canonical source path used for the check.
    pub path: Utf8PathBuf,
    /// Number of tokens produced by the current frontend slice.
    pub token_count: usize,
}

/// Summary returned by a successful `fscript run`.
#[derive(Clone, Debug, PartialEq)]
pub struct RunSummary {
    /// Canonical source path used for execution.
    pub path: Utf8PathBuf,
    /// Final value produced by the last top-level binding in the current execution slice.
    pub last_value: Option<Value>,
}

/// Structured source diagnostic surfaced to browser and editor integrations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    /// Machine-readable diagnostic category.
    pub kind: &'static str,
    /// Human-readable headline for the diagnostic.
    pub title: String,
    /// Full single-line message.
    pub message: String,
    /// One-based source line when available.
    pub line: Option<usize>,
    /// One-based source column when available.
    pub column: Option<usize>,
    /// Highlight width when available.
    pub width: Option<usize>,
    /// Pretty-printed source location when available.
    pub location: Option<String>,
    /// Label attached to the highlighted span.
    pub label: Option<String>,
}

/// Runs the current `check` pipeline for a single file.
pub fn check_file(path: &Utf8Path) -> Result<CheckSummary, DriverError> {
    let program = load_program(path)?;

    Ok(CheckSummary {
        path: program.entry_path.clone(),
        token_count: program.entry_token_count,
    })
}

/// Runs the first executable FScript slice.
pub fn run_file(path: &Utf8Path) -> Result<RunSummary, DriverError> {
    let program = load_program(path)?;
    let modules = program
        .modules
        .iter()
        .map(|(path, module)| (path.as_str().to_owned(), module.ir.clone()))
        .collect::<BTreeMap<_, _>>();
    let last_value = run_modules(&modules, program.entry_path.as_str())?;

    Ok(RunSummary {
        path: program.entry_path,
        last_value,
    })
}

/// Runs the compiler pipeline against an in-memory source string.
pub fn run_source(source_text: &str) -> Result<RunSummary, DriverError> {
    let path = Utf8PathBuf::from("sandbox.fs");
    let module = load_module_from_source(path.clone(), source_text.to_owned())?;
    reject_non_std_imports(&module.ir)?;
    let last_value = fscript_interpreter::run_module(&module.ir)
        .map(|value| value.map(Value::from_runtime_value))
        .map_err(|error| DriverError::from(RuntimeFailed::from_message(error.message())))?;

    Ok(RunSummary { path, last_value })
}

/// Checks an in-memory source string without touching the filesystem.
pub fn check_source(source_text: &str) -> Result<CheckSummary, DriverError> {
    let path = Utf8PathBuf::from("sandbox.fs");
    let module = load_module_from_source(path.clone(), source_text.to_owned())?;

    Ok(CheckSummary {
        path,
        token_count: module.token_count,
    })
}

/// Compiles the current executable subset into a native binary.
pub fn compile_file(input: &Utf8Path, output: &Utf8Path) -> Result<(), DriverError> {
    let program = load_program(input)?;
    let modules = program
        .modules
        .iter()
        .map(|(path, module)| (path.as_str().to_owned(), module.ir.clone()))
        .collect::<BTreeMap<_, _>>();
    fscript_codegen_cranelift::compile_program(&modules, program.entry_path.as_str(), output)
        .map_err(|error| DriverError::from(CompileFailed::from_program(&program, error)))
}

#[derive(Clone, Debug)]
struct LoadedModule {
    source: SourceFile,
    ast: Module,
    hir: hir::Module,
    ir: fscript_ir::Module,
    token_count: usize,
}

#[derive(Clone, Debug)]
struct LoadedProgram {
    entry_path: Utf8PathBuf,
    entry_token_count: usize,
    modules: BTreeMap<Utf8PathBuf, LoadedModule>,
}

fn validate_source_extension(path: &Utf8Path) -> Result<(), DriverError> {
    if path.extension() == Some("fs") {
        Ok(())
    } else {
        Err(DriverError::UnsupportedExtension(path.to_owned()))
    }
}

/// Driver-level failures surfaced to the CLI.
#[derive(Debug, Error, Diagnostic)]
pub enum DriverError {
    #[error(transparent)]
    Source(#[from] SourceLoadError),
    #[error("expected an `.fs` source file, got `{0}`")]
    UnsupportedExtension(Utf8PathBuf),
    #[error(transparent)]
    Import(#[from] ImportFailed),
    #[error(transparent)]
    Lex(Box<LexFailed>),
    #[error(transparent)]
    Parse(Box<ParseFailed>),
    #[error(transparent)]
    Lower(Box<LowerFailed>),
    #[error(transparent)]
    Type(Box<TypeFailed>),
    #[error(transparent)]
    Effect(Box<EffectFailed>),
    #[error(transparent)]
    Runtime(Box<RuntimeFailed>),
    #[error(transparent)]
    Compile(Box<CompileFailed>),
}

impl From<LexFailed> for DriverError {
    fn from(value: LexFailed) -> Self {
        Self::Lex(Box::new(value))
    }
}

impl From<ParseFailed> for DriverError {
    fn from(value: ParseFailed) -> Self {
        Self::Parse(Box::new(value))
    }
}

impl From<LowerFailed> for DriverError {
    fn from(value: LowerFailed) -> Self {
        Self::Lower(Box::new(value))
    }
}

impl From<TypeFailed> for DriverError {
    fn from(value: TypeFailed) -> Self {
        Self::Type(Box::new(value))
    }
}

impl From<EffectFailed> for DriverError {
    fn from(value: EffectFailed) -> Self {
        Self::Effect(Box::new(value))
    }
}

impl From<RuntimeFailed> for DriverError {
    fn from(value: RuntimeFailed) -> Self {
        Self::Runtime(Box::new(value))
    }
}

impl From<CompileFailed> for DriverError {
    fn from(value: CompileFailed) -> Self {
        Self::Compile(Box::new(value))
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
pub struct ImportFailed {
    message: String,
}

impl ImportFailed {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct LexFailed {
    message: String,
    title: String,
    location: String,
    line_number: usize,
    source_line: String,
    pointer_column: usize,
    pointer_width: usize,
    pointer_label: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
    src: NamedSource<String>,
    label: String,
    span: SourceSpan,
    _related: Vec<LexReport>,
}

impl LexFailed {
    fn from_source(source: &SourceFile, diagnostics: Vec<LexDiagnostic>) -> Self {
        let mut reports = diagnostics
            .into_iter()
            .map(|diagnostic| LexReport::new(source, diagnostic))
            .collect::<Vec<_>>();
        let primary = reports.remove(0);
        let location = format!("{}:{}:{}", source.path(), primary.line, primary.column);

        Self {
            message: format!("{} at {}", primary.message, location),
            title: primary.message.clone(),
            location,
            line_number: primary.line,
            source_line: primary.source_line.clone(),
            pointer_column: primary.column,
            pointer_width: primary.pointer_width,
            pointer_label: primary.pointer_label.clone(),
            before_lines: primary.before_lines.clone(),
            after_lines: primary.after_lines.clone(),
            src: primary.src.clone(),
            label: primary.label.clone(),
            span: primary.span,
            _related: reports,
        }
    }

    fn render_context(&self) -> SourceRenderContext<'_> {
        SourceRenderContext {
            title: &self.title,
            location: &self.location,
            line_number: self.line_number,
            before_lines: &self.before_lines,
            source_line: &self.source_line,
            pointer_column: self.pointer_column,
            pointer_width: self.pointer_width,
            pointer_label: &self.pointer_label,
            after_lines: &self.after_lines,
        }
    }
}

#[derive(Clone, Debug, Error)]
#[error("{message}")]
struct LexReport {
    message: String,
    line: usize,
    column: usize,
    source_line: String,
    pointer_width: usize,
    pointer_label: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
    src: NamedSource<String>,
    label: String,
    span: SourceSpan,
}

impl LexReport {
    fn new(source: &SourceFile, diagnostic: LexDiagnostic) -> Self {
        let (message, label) = match diagnostic.kind {
            LexDiagnosticKind::InvalidToken(value) => (
                format!("invalid token `{value}`"),
                "this character is not valid in FScript".to_owned(),
            ),
            LexDiagnosticKind::UnterminatedString => (
                "unterminated string literal".to_owned(),
                "string literal ends before its closing quote".to_owned(),
            ),
            LexDiagnosticKind::InvalidEscape(value) => (
                format!("invalid escape sequence `\\{value}`"),
                "unsupported escape sequence".to_owned(),
            ),
            LexDiagnosticKind::UnterminatedBlockComment => (
                "unterminated block comment".to_owned(),
                "block comment is missing its closing `*/`".to_owned(),
            ),
        };
        let (line, column) = source.line_column(diagnostic.span.start());
        let source_line = highlight_source_line(source.line_text(line));
        let pointer_width = diagnostic.span.len().clamp(1, 24);
        let pointer_label = "problem starts here".to_owned();
        let before_lines = collect_before_lines(source, line, 1);
        let after_lines = collect_after_lines(source, line, 2);

        Self {
            message,
            line,
            column,
            source_line,
            pointer_width,
            pointer_label,
            before_lines,
            after_lines,
            src: source.named_source(),
            label,
            span: SourceSpan::from(diagnostic.span),
        }
    }
}

impl Diagnostic for LexReport {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.clone()),
            self.span,
        ))))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }
}

impl Diagnostic for LexFailed {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.clone()),
            self.span,
        ))))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct ParseFailed {
    message: String,
    title: String,
    location: String,
    line_number: usize,
    source_line: String,
    pointer_column: usize,
    pointer_width: usize,
    pointer_label: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
    src: NamedSource<String>,
    label: String,
    span: SourceSpan,
    _related: Vec<ParseReport>,
}

impl ParseFailed {
    fn from_source(source: &SourceFile, diagnostics: Vec<ParseDiagnostic>) -> Self {
        let mut reports = diagnostics
            .into_iter()
            .map(|diagnostic| ParseReport::new(source, diagnostic))
            .collect::<Vec<_>>();
        let primary = reports.remove(0);
        let location = format!("{}:{}:{}", source.path(), primary.line, primary.column);

        Self {
            message: format!("{} at {}", primary.message, location),
            title: primary.message.clone(),
            location,
            line_number: primary.line,
            source_line: primary.source_line.clone(),
            pointer_column: primary.column,
            pointer_width: primary.pointer_width,
            pointer_label: primary.pointer_label.clone(),
            before_lines: primary.before_lines.clone(),
            after_lines: primary.after_lines.clone(),
            src: primary.src.clone(),
            label: primary.label.clone(),
            span: primary.span,
            _related: reports,
        }
    }

    fn render_context(&self) -> SourceRenderContext<'_> {
        SourceRenderContext {
            title: &self.title,
            location: &self.location,
            line_number: self.line_number,
            before_lines: &self.before_lines,
            source_line: &self.source_line,
            pointer_column: self.pointer_column,
            pointer_width: self.pointer_width,
            pointer_label: &self.pointer_label,
            after_lines: &self.after_lines,
        }
    }
}

#[derive(Clone, Debug, Error)]
#[error("{message}")]
struct ParseReport {
    message: String,
    line: usize,
    column: usize,
    source_line: String,
    pointer_width: usize,
    pointer_label: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
    src: NamedSource<String>,
    label: String,
    span: SourceSpan,
}

impl ParseReport {
    fn new(source: &SourceFile, diagnostic: ParseDiagnostic) -> Self {
        let (message, label) = match diagnostic.kind {
            ParseDiagnosticKind::Expected(expected) => (
                format!("expected {expected}"),
                format!("the parser expected {expected} at this location"),
            ),
            ParseDiagnosticKind::UnexpectedToken(kind) => (
                format!("unexpected token `{:?}`", kind),
                "this token does not fit the current FScript grammar".to_owned(),
            ),
            ParseDiagnosticKind::InvalidModuleItem => (
                "invalid module item".to_owned(),
                "top-level code must be an import, export, type declaration, or binding".to_owned(),
            ),
            ParseDiagnosticKind::InvalidPattern => (
                "invalid pattern".to_owned(),
                "expected an identifier, destructuring pattern, or literal pattern".to_owned(),
            ),
            ParseDiagnosticKind::InvalidType => (
                "invalid type".to_owned(),
                "expected a valid FScript type expression".to_owned(),
            ),
        };
        let (line, column) = source.line_column(diagnostic.span.start());
        let source_line = highlight_source_line(source.line_text(line));
        let pointer_width = diagnostic.span.len().clamp(1, 24);
        let pointer_label = "unexpected here".to_owned();
        let before_lines = collect_before_lines(source, line, 1);
        let after_lines = collect_after_lines(source, line, 2);

        Self {
            message,
            line,
            column,
            source_line,
            pointer_width,
            pointer_label,
            before_lines,
            after_lines,
            src: source.named_source(),
            label,
            span: SourceSpan::from(diagnostic.span),
        }
    }
}

impl Diagnostic for ParseReport {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.clone()),
            self.span,
        ))))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }
}

impl Diagnostic for ParseFailed {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.clone()),
            self.span,
        ))))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct LowerFailed {
    message: String,
    title: String,
    location: String,
    line_number: usize,
    source_line: String,
    pointer_column: usize,
    pointer_width: usize,
    pointer_label: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
    src: NamedSource<String>,
    label: String,
    span: SourceSpan,
}

impl LowerFailed {
    fn from_source(source: &SourceFile, error: LowerError) -> Self {
        let span = error.span();
        let (line, column) = source.line_column(span.start());
        let title = error.to_string();
        let location = format!("{}:{}:{}", source.path(), line, column);

        Self {
            message: format!("{title} at {location}"),
            title,
            location,
            line_number: line,
            source_line: highlight_source_line(source.line_text(line)),
            pointer_column: column,
            pointer_width: span.len().clamp(1, 24),
            pointer_label: "resolution failed here".to_owned(),
            before_lines: collect_before_lines(source, line, 1),
            after_lines: collect_after_lines(source, line, 2),
            src: source.named_source(),
            label: "the semantic frontend could not resolve this name".to_owned(),
            span: SourceSpan::from(span),
        }
    }

    fn render_context(&self) -> SourceRenderContext<'_> {
        SourceRenderContext {
            title: &self.title,
            location: &self.location,
            line_number: self.line_number,
            before_lines: &self.before_lines,
            source_line: &self.source_line,
            pointer_column: self.pointer_column,
            pointer_width: self.pointer_width,
            pointer_label: &self.pointer_label,
            after_lines: &self.after_lines,
        }
    }
}

impl Diagnostic for LowerFailed {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.clone()),
            self.span,
        ))))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct TypeFailed {
    message: String,
    title: String,
    location: String,
    line_number: usize,
    source_line: String,
    pointer_column: usize,
    pointer_width: usize,
    pointer_label: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
    src: NamedSource<String>,
    label: String,
    span: SourceSpan,
}

impl TypeFailed {
    fn from_source(source: &SourceFile, error: TypeError) -> Self {
        let span = error.span();
        let (line, column) = source.line_column(span.start());
        let title = error.message().to_owned();
        let location = format!("{}:{}:{}", source.path(), line, column);

        Self {
            message: format!("{title} at {location}"),
            title,
            location,
            line_number: line,
            source_line: highlight_source_line(source.line_text(line)),
            pointer_column: column,
            pointer_width: span.len().clamp(1, 24),
            pointer_label: "type mismatch here".to_owned(),
            before_lines: collect_before_lines(source, line, 1),
            after_lines: collect_after_lines(source, line, 2),
            src: source.named_source(),
            label: "the semantic frontend rejected this type relationship".to_owned(),
            span: SourceSpan::from(span),
        }
    }

    fn render_context(&self) -> SourceRenderContext<'_> {
        SourceRenderContext {
            title: &self.title,
            location: &self.location,
            line_number: self.line_number,
            before_lines: &self.before_lines,
            source_line: &self.source_line,
            pointer_column: self.pointer_column,
            pointer_width: self.pointer_width,
            pointer_label: &self.pointer_label,
            after_lines: &self.after_lines,
        }
    }
}

impl Diagnostic for TypeFailed {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.clone()),
            self.span,
        ))))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct EffectFailed {
    message: String,
    title: String,
    location: String,
    line_number: usize,
    source_line: String,
    pointer_column: usize,
    pointer_width: usize,
    pointer_label: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
    src: NamedSource<String>,
    label: String,
    span: SourceSpan,
}

impl EffectFailed {
    fn from_source(source: &SourceFile, error: EffectError) -> Self {
        let span = error.span();
        let (line, column) = source.line_column(span.start());
        let title = error.message().to_owned();
        let location = format!("{}:{}:{}", source.path(), line, column);

        Self {
            message: format!("{title} at {location}"),
            title,
            location,
            line_number: line,
            source_line: highlight_source_line(source.line_text(line)),
            pointer_column: column,
            pointer_width: span.len().clamp(1, 24),
            pointer_label: "effect boundary here".to_owned(),
            before_lines: collect_before_lines(source, line, 1),
            after_lines: collect_after_lines(source, line, 2),
            src: source.named_source(),
            label: "the effect analyzer rejected this expression".to_owned(),
            span: SourceSpan::from(span),
        }
    }

    fn render_context(&self) -> SourceRenderContext<'_> {
        SourceRenderContext {
            title: &self.title,
            location: &self.location,
            line_number: self.line_number,
            before_lines: &self.before_lines,
            source_line: &self.source_line,
            pointer_column: self.pointer_column,
            pointer_width: self.pointer_width,
            pointer_label: &self.pointer_label,
            after_lines: &self.after_lines,
        }
    }
}

impl Diagnostic for EffectFailed {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
            Some(self.label.clone()),
            self.span,
        ))))
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.src)
    }
}

impl DriverError {
    /// Returns structured diagnostic details when the error is source-related.
    #[must_use]
    pub fn diagnostic_summary(&self) -> DiagnosticSummary {
        match self {
            Self::Source(error) => DiagnosticSummary {
                kind: "source",
                title: error.to_string(),
                message: error.to_string(),
                line: None,
                column: None,
                width: None,
                location: None,
                label: None,
            },
            Self::UnsupportedExtension(path) => DiagnosticSummary {
                kind: "source",
                title: "unsupported source extension".to_owned(),
                message: format!("expected an `.fs` source file, got `{path}`"),
                line: None,
                column: None,
                width: None,
                location: Some(path.to_string()),
                label: None,
            },
            Self::Import(error) => DiagnosticSummary {
                kind: "import",
                title: error.to_string(),
                message: error.to_string(),
                line: None,
                column: None,
                width: None,
                location: None,
                label: None,
            },
            Self::Lex(error) => DiagnosticSummary {
                kind: "lex",
                title: error.title.clone(),
                message: error.message.clone(),
                line: Some(error.line_number),
                column: Some(error.pointer_column),
                width: Some(error.pointer_width),
                location: Some(error.location.clone()),
                label: Some(error.label.clone()),
            },
            Self::Parse(error) => DiagnosticSummary {
                kind: "parse",
                title: error.title.clone(),
                message: error.message.clone(),
                line: Some(error.line_number),
                column: Some(error.pointer_column),
                width: Some(error.pointer_width),
                location: Some(error.location.clone()),
                label: Some(error.label.clone()),
            },
            Self::Lower(error) => DiagnosticSummary {
                kind: "lower",
                title: error.title.clone(),
                message: error.message.clone(),
                line: Some(error.line_number),
                column: Some(error.pointer_column),
                width: Some(error.pointer_width),
                location: Some(error.location.clone()),
                label: Some(error.label.clone()),
            },
            Self::Type(error) => DiagnosticSummary {
                kind: "type",
                title: error.title.clone(),
                message: error.message.clone(),
                line: Some(error.line_number),
                column: Some(error.pointer_column),
                width: Some(error.pointer_width),
                location: Some(error.location.clone()),
                label: Some(error.label.clone()),
            },
            Self::Effect(error) => DiagnosticSummary {
                kind: "effect",
                title: error.title.clone(),
                message: error.message.clone(),
                line: Some(error.line_number),
                column: Some(error.pointer_column),
                width: Some(error.pointer_width),
                location: Some(error.location.clone()),
                label: Some(error.label.clone()),
            },
            Self::Runtime(error) => DiagnosticSummary {
                kind: "runtime",
                title: "runtime evaluation failed".to_owned(),
                message: error
                    .diagnostics
                    .first()
                    .map(|diagnostic| diagnostic.message.clone())
                    .unwrap_or_else(|| error.to_string()),
                line: None,
                column: None,
                width: None,
                location: None,
                label: None,
            },
            Self::Compile(error) => match error.as_ref() {
                CompileFailed::Source(error) => DiagnosticSummary {
                    kind: "compile",
                    title: error.title.clone(),
                    message: error.message.clone(),
                    line: Some(error.line_number),
                    column: Some(error.pointer_column),
                    width: Some(error.pointer_width),
                    location: Some(error.location.clone()),
                    label: Some(error.label.clone()),
                },
                CompileFailed::Tool { message } => DiagnosticSummary {
                    kind: "compile",
                    title: message.clone(),
                    message: message.clone(),
                    line: None,
                    column: None,
                    width: None,
                    location: None,
                    label: None,
                },
            },
        }
    }

    /// Formats a compiler-style error message for terminal output.
    #[must_use]
    pub fn render_pretty(&self) -> String {
        match self {
            Self::Lex(error) => render_source_error(error.render_context()),
            Self::Parse(error) => render_source_error(error.render_context()),
            Self::Lower(error) => render_source_error(error.render_context()),
            Self::Type(error) => render_source_error(error.render_context()),
            Self::Effect(error) => render_source_error(error.render_context()),
            Self::Compile(error) => error.render_pretty(),
            _ => format!("  × {self}"),
        }
    }
}

#[derive(Debug, Error)]
pub enum CompileFailed {
    #[error(transparent)]
    Source(Box<CompileSourceFailed>),
    #[error("{message}")]
    Tool { message: String },
}

#[derive(Debug, Error)]
#[error("{message}")]
pub struct CompileSourceFailed {
    message: String,
    title: String,
    location: String,
    line_number: usize,
    source_line: String,
    pointer_column: usize,
    pointer_width: usize,
    pointer_label: String,
    before_lines: Vec<(usize, String)>,
    after_lines: Vec<(usize, String)>,
    src: NamedSource<String>,
    label: String,
    span: SourceSpan,
}

impl CompileFailed {
    fn from_message(message: impl Into<String>) -> Self {
        Self::Tool {
            message: message.into(),
        }
    }

    fn from_program(program: &LoadedProgram, error: CompileError) -> Self {
        let Some(span) = error.span() else {
            return Self::Tool {
                message: error.to_string(),
            };
        };

        let source = error
            .module()
            .and_then(|module| program.modules.get(Utf8Path::new(module)))
            .map(|module| &module.source)
            .or_else(|| {
                program
                    .modules
                    .get(&program.entry_path)
                    .map(|module| &module.source)
            });
        let Some(source) = source else {
            return Self::Tool {
                message: error.to_string(),
            };
        };

        let (line, column) = source.line_column(span.start());
        let source_line = highlight_source_line(source.line_text(line));
        let pointer_width = span.len().clamp(1, 24);
        let title = error.to_string();
        let location = format!("{}:{}:{}", source.path(), line, column);

        Self::Source(Box::new(CompileSourceFailed {
            message: format!("{title} at {location}"),
            title,
            location,
            line_number: line,
            source_line,
            pointer_column: column,
            pointer_width,
            pointer_label: "unsupported here".to_owned(),
            before_lines: collect_before_lines(source, line, 1),
            after_lines: collect_after_lines(source, line, 2),
            src: source.named_source(),
            label: "the bootstrap compiler cannot lower this source construct yet".to_owned(),
            span: SourceSpan::from(span),
        }))
    }

    fn render_pretty(&self) -> String {
        match self {
            Self::Source(error) => render_source_error(SourceRenderContext {
                title: &error.title,
                location: &error.location,
                line_number: error.line_number,
                before_lines: &error.before_lines,
                source_line: &error.source_line,
                pointer_column: error.pointer_column,
                pointer_width: error.pointer_width,
                pointer_label: &error.pointer_label,
                after_lines: &error.after_lines,
            }),
            Self::Tool { message } => format!("  × {message}"),
        }
    }
}

impl Diagnostic for CompileFailed {
    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        match self {
            Self::Source(error) => Some(Box::new(std::iter::once(LabeledSpan::new_with_span(
                Some(error.label.clone()),
                error.span,
            )))),
            Self::Tool { .. } => None,
        }
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        match self {
            Self::Source(error) => Some(&error.src),
            Self::Tool { .. } => None,
        }
    }
}

fn load_program(path: &Utf8Path) -> Result<LoadedProgram, DriverError> {
    let entry_path = canonicalize_source_path(path)?;
    let mut modules = BTreeMap::new();
    let mut visiting = BTreeSet::new();
    load_program_module(&entry_path, &mut modules, &mut visiting)?;
    let entry_token_count = modules
        .get(&entry_path)
        .map(|module| module.token_count)
        .ok_or_else(|| ImportFailed::new(format!("entry module `{entry_path}` was not loaded")))?;

    Ok(LoadedProgram {
        entry_path,
        entry_token_count,
        modules,
    })
}

fn load_program_module(
    path: &Utf8Path,
    modules: &mut BTreeMap<Utf8PathBuf, LoadedModule>,
    visiting: &mut BTreeSet<Utf8PathBuf>,
) -> Result<(), DriverError> {
    if modules.contains_key(path) {
        return Ok(());
    }

    if !visiting.insert(path.to_owned()) {
        return Err(ImportFailed::new(format!("circular import detected at `{path}`")).into());
    }

    let mut module = load_single_module(path)?;
    let parent = path.parent().unwrap_or_else(|| Utf8Path::new("."));

    for item in &mut module.ir.items {
        let fscript_ir::ModuleItem::Import(import) = item else {
            continue;
        };

        if import.source.starts_with("std:") {
            continue;
        }

        let resolved = resolve_import_path(parent, &import.source)?;
        load_program_module(&resolved, modules, visiting)?;
        import.source = resolved.as_str().to_owned();
    }

    visiting.remove(path);
    modules.insert(path.to_owned(), module);
    Ok(())
}

fn load_single_module(path: &Utf8Path) -> Result<LoadedModule, DriverError> {
    validate_source_extension(path)?;

    let source = SourceFile::load(path)?;
    load_module_from_source_file(source)
}

fn load_module_from_source(
    path: Utf8PathBuf,
    contents: String,
) -> Result<LoadedModule, DriverError> {
    validate_source_extension(&path)?;
    let source = SourceFile::new(path, contents);
    load_module_from_source_file(source)
}

fn load_module_from_source_file(source: SourceFile) -> Result<LoadedModule, DriverError> {
    let lexed = lex(&source);

    if !lexed.diagnostics.is_empty() {
        return Err(DriverError::from(LexFailed::from_source(
            &source,
            lexed.diagnostics,
        )));
    }

    let token_count = lexed.tokens.len();
    let parsed = parse_module(&source, &lexed.tokens);
    if !parsed.diagnostics.is_empty() {
        return Err(DriverError::from(ParseFailed::from_source(
            &source,
            parsed.diagnostics,
        )));
    }

    let hir = fscript_lower::lower_module(&parsed.module)
        .map_err(|error| DriverError::from(LowerFailed::from_source(&source, error)))?;
    fscript_types::check_module(&hir)
        .map_err(|error| DriverError::from(TypeFailed::from_source(&source, error)))?;
    fscript_effects::analyze_module(&hir)
        .map_err(|error| DriverError::from(EffectFailed::from_source(&source, error)))?;
    let ir = fscript_lower::lower_to_ir(&hir);

    Ok(LoadedModule {
        source,
        ast: parsed.module,
        hir,
        ir,
        token_count,
    })
}

fn run_modules(
    modules: &BTreeMap<String, fscript_ir::Module>,
    entry: &str,
) -> Result<Option<Value>, DriverError> {
    fscript_interpreter::run_program(modules, entry)
        .map(|value| value.map(Value::from_runtime_value))
        .map_err(|error| DriverError::from(RuntimeFailed::from_message(error.message())))
}

fn reject_non_std_imports(module: &fscript_ir::Module) -> Result<(), DriverError> {
    for item in &module.items {
        let fscript_ir::ModuleItem::Import(import) = item else {
            continue;
        };

        if !import.source.starts_with("std:") {
            return Err(DriverError::Import(ImportFailed::new(format!(
                "the browser sandbox currently supports standard-library imports only, found `{}`",
                import.source
            ))));
        }
    }

    Ok(())
}

fn canonicalize_source_path(path: &Utf8Path) -> Result<Utf8PathBuf, DriverError> {
    validate_source_extension(path)?;
    let path = fs::canonicalize(path).map_err(|source| {
        DriverError::Source(SourceLoadError::Read {
            path: path.to_owned(),
            source,
        })
    })?;
    Utf8PathBuf::from_path_buf(path).map_err(|path| {
        ImportFailed::new(format!(
            "source path `{}` is not valid UTF-8",
            path.to_string_lossy()
        ))
        .into()
    })
}

fn resolve_import_path(base: &Utf8Path, source: &str) -> Result<Utf8PathBuf, DriverError> {
    if !source.starts_with("./") && !source.starts_with("../") {
        return Err(ImportFailed::new(format!(
            "only relative user-module imports are supported, found `{source}`"
        ))
        .into());
    }

    let resolved = base.join(source);
    canonicalize_source_path(&resolved)
}

struct SourceRenderContext<'a> {
    title: &'a str,
    location: &'a str,
    line_number: usize,
    before_lines: &'a [(usize, String)],
    source_line: &'a str,
    pointer_column: usize,
    pointer_width: usize,
    pointer_label: &'a str,
    after_lines: &'a [(usize, String)],
}

fn render_source_error(context: SourceRenderContext<'_>) -> String {
    const RED: &str = "\u{1b}[31m";
    const CYAN: &str = "\u{1b}[36m";
    const DIM: &str = "\u{1b}[90m";
    const BOLD: &str = "\u{1b}[1m";
    const RESET: &str = "\u{1b}[0m";

    let gutter_width = context
        .before_lines
        .iter()
        .map(|(line, _)| *line)
        .chain(std::iter::once(context.line_number))
        .chain(context.after_lines.iter().map(|(line, _)| *line))
        .max()
        .unwrap_or(context.line_number)
        .to_string()
        .len();

    let mut lines = vec![
        format!("  {RED}×{RESET} {BOLD}{}{RESET}", context.title),
        format!("    {DIM}at {CYAN}{}{RESET}", context.location),
    ];

    for (line, text) in context.before_lines {
        lines.push(format!(
            "    {DIM}{line:>width$} | {text}{RESET}",
            width = gutter_width
        ));
    }

    lines.push(format!(
        "    {BOLD}{:>width$} | {}{RESET}",
        context.line_number,
        context.source_line,
        width = gutter_width
    ));
    lines.push(format!(
        "    {DIM}{:>width$} | {RESET}{}{RED}{}{RESET} {DIM}{}{RESET}",
        "",
        " ".repeat(context.pointer_column.saturating_sub(1)),
        "^".repeat(context.pointer_width),
        context.pointer_label,
        width = gutter_width
    ));

    for (line, text) in context.after_lines {
        lines.push(format!(
            "    {DIM}{line:>width$} | {text}{RESET}",
            width = gutter_width
        ));
    }

    lines.join("\n")
}

fn collect_before_lines(source: &SourceFile, line: usize, count: usize) -> Vec<(usize, String)> {
    let start = line.saturating_sub(count);
    (start..line)
        .filter(|line_number| *line_number >= 1)
        .map(|line_number| (line_number, source.line_text(line_number).to_owned()))
        .collect()
}

fn collect_after_lines(source: &SourceFile, line: usize, count: usize) -> Vec<(usize, String)> {
    ((line + 1)..=(line + count))
        .filter(|line_number| *line_number <= source.line_count())
        .filter_map(|line_number| {
            let text = source.line_text(line_number);
            if text.is_empty() && line_number == source.line_count() {
                None
            } else {
                Some((line_number, text.to_owned()))
            }
        })
        .collect()
}

fn highlight_source_line(line: &str) -> String {
    const GREEN: &str = "\u{1b}[32m";
    const DIM: &str = "\u{1b}[90m";
    const RESET: &str = "\u{1b}[0m";

    let mut highlighted = String::new();
    let mut chars = line.chars().peekable();
    let mut in_string = false;
    let mut string_delimiter = '\0';

    while let Some(ch) = chars.next() {
        if !in_string && ch == '/' && chars.peek() == Some(&'/') {
            highlighted.push_str(DIM);
            highlighted.push(ch);
            highlighted.push(chars.next().expect("peeked comment slash"));
            highlighted.push_str(chars.collect::<String>().as_str());
            highlighted.push_str(RESET);
            break;
        }

        if !in_string && matches!(ch, '\'' | '"') {
            in_string = true;
            string_delimiter = ch;
            highlighted.push_str(GREEN);
            highlighted.push(ch);
            continue;
        }

        highlighted.push(ch);

        if in_string && ch == string_delimiter {
            in_string = false;
            string_delimiter = '\0';
            highlighted.push_str(RESET);
        }
    }

    if in_string {
        highlighted.push_str(RESET);
    }

    highlighted
}

#[derive(Debug, Error, Diagnostic)]
#[error("runtime evaluation failed")]
pub struct RuntimeFailed {
    #[related]
    diagnostics: Vec<RuntimeReport>,
}

impl RuntimeFailed {
    fn from_message(message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: message.into(),
            }],
        }
    }

    fn unknown_identifier(name: String) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("unknown identifier `{name}`"),
            }],
        }
    }

    fn duplicate_binding(name: String) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("binding `{name}` is already defined in this scope"),
            }],
        }
    }

    fn unknown_std_module(source: &str) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("unknown standard library module `{source}`"),
            }],
        }
    }

    fn unsupported_import(source: &str) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("runtime imports from `{source}` are not implemented yet"),
            }],
        }
    }

    fn unknown_export(source: &str, name: &str) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("module `{source}` does not export `{name}`"),
            }],
        }
    }

    fn missing_property(name: &str) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("record does not contain a `{name}` field"),
            }],
        }
    }

    fn unsupported(feature: &'static str) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("runtime support for {feature} is not implemented yet"),
            }],
        }
    }

    fn unsupported_call(value: &Value) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("cannot call value `{value}`"),
            }],
        }
    }

    fn type_mismatch(message: impl Into<String>) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: message.into(),
            }],
        }
    }

    fn no_match_arm() -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: "match expression did not find a matching arm".to_owned(),
            }],
        }
    }

    fn uncaught_throw(value: Value) -> Self {
        Self {
            diagnostics: vec![RuntimeReport {
                message: format!("uncaught thrown value `{value}`"),
            }],
        }
    }
}

#[derive(Debug, Error, Diagnostic)]
#[error("{message}")]
struct RuntimeReport {
    message: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    String(String),
    Number(f64),
    Boolean(bool),
    Null,
    Undefined,
    Record(BTreeMap<String, Value>),
    Array(Vec<Value>),
    Sequence(Vec<Value>),
    Deferred(DeferredValue),
    Function(FunctionValue),
    NativeFunction(NativeFunctionValue),
}

impl Value {
    fn from_runtime_value(value: shared_runtime::Value) -> Self {
        match value {
            shared_runtime::Value::String(value) => Self::String(value),
            shared_runtime::Value::Number(value) => Self::Number(value),
            shared_runtime::Value::Boolean(value) => Self::Boolean(value),
            shared_runtime::Value::Null => Self::Null,
            shared_runtime::Value::Undefined => Self::Undefined,
            shared_runtime::Value::Record(fields) => Self::Record(BTreeMap::from_iter(
                fields
                    .into_iter()
                    .map(|(name, value)| (name, Self::from_runtime_value(value))),
            )),
            shared_runtime::Value::Array(items) => {
                Self::Array(items.into_iter().map(Self::from_runtime_value).collect())
            }
            shared_runtime::Value::Sequence(items) => {
                Self::Sequence(items.into_iter().map(Self::from_runtime_value).collect())
            }
            shared_runtime::Value::Deferred(_) => Self::Deferred(DeferredValue {
                expr: Box::new(Expr::Undefined {
                    span: fscript_source::Span::new(0, 0),
                }),
                environment: Environment::new(),
                resolved: Rc::new(RefCell::new(None)),
            }),
            shared_runtime::Value::Function(function) => Self::Function(FunctionValue {
                parameters: function
                    .parameters
                    .into_iter()
                    .map(|parameter| Parameter {
                        pattern: Pattern::Identifier(fscript_ast::Identifier {
                            name: "[runtime]".to_owned(),
                            span: parameter.span,
                        }),
                        type_annotation: None,
                        span: parameter.span,
                    })
                    .collect(),
                body: Box::new(Expr::Undefined {
                    span: fscript_source::Span::new(0, 0),
                }),
                environment: Environment::new(),
                applied_args: Vec::new(),
                is_generator: function.is_generator,
            }),
            shared_runtime::Value::NativeFunction(function) => {
                Self::NativeFunction(NativeFunctionValue::new(match function.function {
                    shared_runtime::NativeFunction::ObjectSpread => NativeFunction::ObjectSpread,
                    shared_runtime::NativeFunction::ArrayMap => NativeFunction::ArrayMap,
                    shared_runtime::NativeFunction::ArrayFilter => NativeFunction::ArrayFilter,
                    shared_runtime::NativeFunction::ArrayLength => NativeFunction::ArrayLength,
                    shared_runtime::NativeFunction::HttpServe => NativeFunction::HttpServe,
                    shared_runtime::NativeFunction::JsonToObject => NativeFunction::JsonToObject,
                    shared_runtime::NativeFunction::JsonToString => NativeFunction::JsonToString,
                    shared_runtime::NativeFunction::JsonToPrettyString => {
                        NativeFunction::JsonToPrettyString
                    }
                    shared_runtime::NativeFunction::LoggerCreate => NativeFunction::LoggerCreate,
                    shared_runtime::NativeFunction::LoggerLog => NativeFunction::LoggerLog,
                    shared_runtime::NativeFunction::LoggerDebug => NativeFunction::LoggerDebug,
                    shared_runtime::NativeFunction::LoggerInfo => NativeFunction::LoggerInfo,
                    shared_runtime::NativeFunction::LoggerWarn => NativeFunction::LoggerWarn,
                    shared_runtime::NativeFunction::LoggerError => NativeFunction::LoggerError,
                    shared_runtime::NativeFunction::LoggerPrettyJson => {
                        NativeFunction::LoggerPrettyJson
                    }
                    shared_runtime::NativeFunction::FilesystemReadFile => {
                        NativeFunction::FilesystemReadFile
                    }
                    shared_runtime::NativeFunction::FilesystemWriteFile => {
                        NativeFunction::FilesystemWriteFile
                    }
                    shared_runtime::NativeFunction::FilesystemExists => {
                        NativeFunction::FilesystemExists
                    }
                    shared_runtime::NativeFunction::FilesystemDeleteFile => {
                        NativeFunction::FilesystemDeleteFile
                    }
                    shared_runtime::NativeFunction::FilesystemReadDir => {
                        NativeFunction::FilesystemReadDir
                    }
                    shared_runtime::NativeFunction::StringTrim => NativeFunction::StringTrim,
                    shared_runtime::NativeFunction::StringUppercase => {
                        NativeFunction::StringUppercase
                    }
                    shared_runtime::NativeFunction::StringLowercase => {
                        NativeFunction::StringLowercase
                    }
                    shared_runtime::NativeFunction::StringIsDigits => {
                        NativeFunction::StringIsDigits
                    }
                    shared_runtime::NativeFunction::NumberParse => NativeFunction::NumberParse,
                    shared_runtime::NativeFunction::ResultOk => NativeFunction::ResultOk,
                    shared_runtime::NativeFunction::ResultError => NativeFunction::ResultError,
                    shared_runtime::NativeFunction::ResultIsOk => NativeFunction::ResultIsOk,
                    shared_runtime::NativeFunction::ResultIsError => NativeFunction::ResultIsError,
                    shared_runtime::NativeFunction::ResultWithDefault => {
                        NativeFunction::ResultWithDefault
                    }
                    shared_runtime::NativeFunction::TaskAll => NativeFunction::TaskAll,
                    shared_runtime::NativeFunction::TaskRace => NativeFunction::TaskRace,
                    shared_runtime::NativeFunction::TaskSpawn => NativeFunction::TaskSpawn,
                    shared_runtime::NativeFunction::TaskDefer => NativeFunction::TaskDefer,
                    shared_runtime::NativeFunction::TaskForce => NativeFunction::TaskForce,
                }))
            }
        }
    }
}

type Environment = BTreeMap<String, Value>;
type YieldValues<'a> = Option<&'a RefCell<Vec<Value>>>;
type RuntimeResult<T> = Result<T, RuntimeFailed>;
type RuntimeEval<T> = Result<T, RuntimeControl>;

#[derive(Clone, Debug, PartialEq)]
enum EvalOutcome {
    Value(Value),
    Throw(Value),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeferredValue {
    expr: Box<Expr>,
    environment: Environment,
    resolved: Rc<RefCell<Option<EvalOutcome>>>,
}

#[derive(Debug)]
enum RuntimeControl {
    Error(RuntimeFailed),
    Throw(Value),
}

impl From<RuntimeFailed> for RuntimeControl {
    fn from(value: RuntimeFailed) -> Self {
        Self::Error(value)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FunctionValue {
    parameters: Vec<Parameter>,
    body: Box<Expr>,
    environment: Environment,
    applied_args: Vec<Value>,
    is_generator: bool,
}

impl FunctionValue {
    fn arity(&self) -> usize {
        self.parameters.len()
    }

    fn with_args(&self, args: Vec<Value>) -> Self {
        Self {
            parameters: self.parameters.clone(),
            body: self.body.clone(),
            environment: self.environment.clone(),
            applied_args: args,
            is_generator: self.is_generator,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct NativeFunctionValue {
    function: NativeFunction,
    applied_args: Vec<Value>,
}

impl NativeFunctionValue {
    fn new(function: NativeFunction) -> Self {
        Self {
            function,
            applied_args: Vec::new(),
        }
    }

    fn with_args(&self, args: Vec<Value>) -> Self {
        Self {
            function: self.function,
            applied_args: args,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NativeFunction {
    ObjectSpread,
    ArrayMap,
    ArrayFilter,
    ArrayLength,
    HttpServe,
    JsonToObject,
    JsonToString,
    JsonToPrettyString,
    LoggerCreate,
    LoggerLog,
    LoggerDebug,
    LoggerInfo,
    LoggerWarn,
    LoggerError,
    LoggerPrettyJson,
    FilesystemReadFile,
    FilesystemWriteFile,
    FilesystemExists,
    FilesystemDeleteFile,
    FilesystemReadDir,
    StringTrim,
    StringUppercase,
    StringLowercase,
    StringIsDigits,
    NumberParse,
    ResultOk,
    ResultError,
    ResultIsOk,
    ResultIsError,
    ResultWithDefault,
    TaskAll,
    TaskRace,
    TaskSpawn,
    TaskDefer,
    TaskForce,
}

impl NativeFunction {
    const fn name(self) -> &'static str {
        match self {
            Self::ObjectSpread => "Object.spread",
            Self::ArrayMap => "Array.map",
            Self::ArrayFilter => "Array.filter",
            Self::ArrayLength => "Array.length",
            Self::HttpServe => "Http.serve",
            Self::JsonToObject => "Json.jsonToObject",
            Self::JsonToString => "Json.jsonToString",
            Self::JsonToPrettyString => "Json.jsonToPrettyString",
            Self::LoggerCreate => "Logger.create",
            Self::LoggerLog => "Logger.log",
            Self::LoggerDebug => "Logger.debug",
            Self::LoggerInfo => "Logger.info",
            Self::LoggerWarn => "Logger.warn",
            Self::LoggerError => "Logger.error",
            Self::LoggerPrettyJson => "Logger.prettyJson",
            Self::FilesystemReadFile => "FileSystem.readFile",
            Self::FilesystemWriteFile => "FileSystem.writeFile",
            Self::FilesystemExists => "FileSystem.exists",
            Self::FilesystemDeleteFile => "FileSystem.deleteFile",
            Self::FilesystemReadDir => "FileSystem.readDir",
            Self::StringTrim => "String.trim",
            Self::StringUppercase => "String.uppercase",
            Self::StringLowercase => "String.lowercase",
            Self::StringIsDigits => "String.isDigits",
            Self::NumberParse => "Number.parse",
            Self::ResultOk => "Result.ok",
            Self::ResultError => "Result.error",
            Self::ResultIsOk => "Result.isOk",
            Self::ResultIsError => "Result.isError",
            Self::ResultWithDefault => "Result.withDefault",
            Self::TaskAll => "Task.all",
            Self::TaskRace => "Task.race",
            Self::TaskSpawn => "Task.spawn",
            Self::TaskDefer => "Task.defer",
            Self::TaskForce => "Task.force",
        }
    }

    const fn arity(self) -> usize {
        match self {
            Self::ObjectSpread => 2,
            Self::ArrayMap | Self::ArrayFilter => 2,
            Self::HttpServe => 2,
            Self::ArrayLength
            | Self::JsonToObject
            | Self::JsonToString
            | Self::JsonToPrettyString
            | Self::LoggerCreate
            | Self::FilesystemReadFile
            | Self::FilesystemExists
            | Self::FilesystemDeleteFile
            | Self::FilesystemReadDir => 1,
            Self::LoggerLog
            | Self::LoggerDebug
            | Self::LoggerInfo
            | Self::LoggerWarn
            | Self::LoggerError
            | Self::LoggerPrettyJson => 2,
            Self::FilesystemWriteFile => 2,
            Self::StringTrim
            | Self::StringUppercase
            | Self::StringLowercase
            | Self::StringIsDigits
            | Self::NumberParse
            | Self::ResultOk
            | Self::ResultError
            | Self::ResultIsOk
            | Self::ResultIsError
            | Self::TaskAll
            | Self::TaskRace
            | Self::TaskSpawn
            | Self::TaskDefer
            | Self::TaskForce => 1,
            Self::ResultWithDefault => 2,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(value) => write!(f, "{value}"),
            Self::Number(value) => write!(f, "{value}"),
            Self::Boolean(value) => write!(f, "{value}"),
            Self::Null => write!(f, "Null"),
            Self::Undefined => write!(f, "Undefined"),
            Self::Record(fields) => {
                write!(f, "{{ ")?;
                let mut first = true;
                for (name, value) in fields {
                    if !first {
                        write!(f, ", ")?;
                    }
                    first = false;
                    write!(f, "{name}: ")?;
                    write_nested_value(f, value)?;
                }
                write!(f, " }}")
            }
            Self::Array(items) => {
                write!(f, "[")?;
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
                    write_nested_value(f, item)?;
                }
                write!(f, "]")
            }
            Self::Sequence(items) => {
                write!(f, "Sequence(")?;
                write_nested_value(f, &Self::Array(items.clone()))?;
                write!(f, ")")
            }
            Self::Deferred(_) => write!(f, "[deferred]"),
            Self::Function(function) => {
                let label = if function.is_generator {
                    "[generator]"
                } else {
                    "[function]"
                };
                write!(f, "{label}")
            }
            Self::NativeFunction(function) => write!(f, "[function {}]", function.function.name()),
        }
    }
}

fn write_nested_value(f: &mut std::fmt::Formatter<'_>, value: &Value) -> std::fmt::Result {
    match value {
        Value::String(string) => write!(f, "'{string}'"),
        other => write!(f, "{other}"),
    }
}

fn evaluate_module(module: &Module) -> Result<Option<Value>, RuntimeFailed> {
    let mut environment = Environment::new();
    let mut last_value = None;

    for item in &module.items {
        match item {
            ModuleItem::Import(import) => load_import(import, &mut environment)?,
            ModuleItem::Type(_) | ModuleItem::ExportType(_) => {}
            ModuleItem::Binding(binding) | ModuleItem::ExportBinding(binding) => {
                let value = evaluate_expr(&binding.value, &environment)?;
                bind_pattern_value(&mut environment, &binding.pattern, value.clone(), None)
                    .map_err(runtime_control_to_failed)?;
                last_value = Some(value);
            }
        }
    }

    last_value
        .map(|value| force_value(value, None).map_err(runtime_control_to_failed))
        .transpose()
}

fn load_import(import: &ImportDecl, environment: &mut Environment) -> Result<(), RuntimeFailed> {
    let module = if import.source.starts_with("std:") {
        load_std_module(&import.source)?
    } else {
        return Err(RuntimeFailed::unsupported_import(&import.source));
    };

    match &import.clause {
        ImportClause::Default(identifier) => define_binding(environment, &identifier.name, module),
        ImportClause::Named(names) => {
            let Value::Record(exports) = &module else {
                return Err(RuntimeFailed::type_mismatch(format!(
                    "module `{}` does not expose named exports",
                    import.source
                )));
            };

            for name in names {
                let value = exports
                    .get(&name.name)
                    .cloned()
                    .ok_or_else(|| RuntimeFailed::unknown_export(&import.source, &name.name))?;
                define_binding(environment, &name.name, value)?;
            }

            Ok(())
        }
    }
}

fn load_std_module(source: &str) -> Result<Value, RuntimeFailed> {
    match source {
        "std:array" => Ok(native_module(&[
            ("map", NativeFunction::ArrayMap),
            ("filter", NativeFunction::ArrayFilter),
            ("length", NativeFunction::ArrayLength),
        ])),
        "std:http" => Ok(native_module(&[("serve", NativeFunction::HttpServe)])),
        "std:json" => Ok(native_module(&[
            ("parse", NativeFunction::JsonToObject),
            ("stringify", NativeFunction::JsonToString),
            ("jsonToObject", NativeFunction::JsonToObject),
            ("jsonToString", NativeFunction::JsonToString),
            ("jsonToPrettyString", NativeFunction::JsonToPrettyString),
        ])),
        "std:logger" => Ok(native_module(&[
            ("create", NativeFunction::LoggerCreate),
            ("log", NativeFunction::LoggerLog),
            ("debug", NativeFunction::LoggerDebug),
            ("info", NativeFunction::LoggerInfo),
            ("warn", NativeFunction::LoggerWarn),
            ("error", NativeFunction::LoggerError),
            ("prettyJson", NativeFunction::LoggerPrettyJson),
        ])),
        "std:object" => Ok(native_module(&[("spread", NativeFunction::ObjectSpread)])),
        "std:string" => Ok(native_module(&[
            ("trim", NativeFunction::StringTrim),
            ("uppercase", NativeFunction::StringUppercase),
            ("lowercase", NativeFunction::StringLowercase),
            ("isDigits", NativeFunction::StringIsDigits),
        ])),
        "std:number" => Ok(native_module(&[("parse", NativeFunction::NumberParse)])),
        "std:result" => Ok(native_module(&[
            ("ok", NativeFunction::ResultOk),
            ("error", NativeFunction::ResultError),
            ("isOk", NativeFunction::ResultIsOk),
            ("isError", NativeFunction::ResultIsError),
            ("withDefault", NativeFunction::ResultWithDefault),
        ])),
        "std:task" => Ok(native_module(&[
            ("all", NativeFunction::TaskAll),
            ("race", NativeFunction::TaskRace),
            ("spawn", NativeFunction::TaskSpawn),
            ("defer", NativeFunction::TaskDefer),
            ("force", NativeFunction::TaskForce),
        ])),
        _ => Err(RuntimeFailed::unknown_std_module(source)),
    }
}

fn native_module(exports: &[(&str, NativeFunction)]) -> Value {
    Value::Record(BTreeMap::from_iter(exports.iter().map(
        |(name, function)| {
            (
                (*name).to_owned(),
                Value::NativeFunction(NativeFunctionValue::new(*function)),
            )
        },
    )))
}

fn define_binding(
    environment: &mut Environment,
    name: &str,
    value: Value,
) -> Result<(), RuntimeFailed> {
    if environment.contains_key(name) {
        return Err(RuntimeFailed::duplicate_binding(name.to_owned()));
    }

    environment.insert(name.to_owned(), value);
    Ok(())
}

fn bind_pattern_value(
    environment: &mut Environment,
    pattern: &Pattern,
    value: Value,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<()> {
    let value = prepare_pattern_value(pattern, value, yield_values)?;
    let Some(bindings) = match_pattern(pattern, &value)? else {
        return Err(RuntimeControl::from(RuntimeFailed::type_mismatch(format!(
            "value `{value}` does not match binding pattern"
        ))));
    };

    for (name, binding_value) in bindings {
        define_binding(environment, &name, binding_value).map_err(RuntimeControl::from)?;
    }

    Ok(())
}

fn match_pattern(pattern: &Pattern, value: &Value) -> Result<Option<Environment>, RuntimeFailed> {
    let mut bindings = Environment::new();
    if pattern_matches(pattern, value, &mut bindings)? {
        Ok(Some(bindings))
    } else {
        Ok(None)
    }
}

fn pattern_matches(
    pattern: &Pattern,
    value: &Value,
    bindings: &mut Environment,
) -> Result<bool, RuntimeFailed> {
    match pattern {
        Pattern::Identifier(identifier) => {
            if bindings.contains_key(&identifier.name) {
                return Err(RuntimeFailed::duplicate_binding(identifier.name.clone()));
            }

            bindings.insert(identifier.name.clone(), value.clone());
            Ok(true)
        }
        Pattern::Literal(literal) => Ok(match literal {
            fscript_ast::LiteralPattern::String {
                value: expected, ..
            } => {
                matches!(value, Value::String(actual) if actual == expected)
            }
            fscript_ast::LiteralPattern::Number {
                value: expected, ..
            } => {
                matches!(value, Value::Number(actual) if actual == expected)
            }
            fscript_ast::LiteralPattern::Boolean {
                value: expected, ..
            } => {
                matches!(value, Value::Boolean(actual) if actual == expected)
            }
            fscript_ast::LiteralPattern::Null { .. } => matches!(value, Value::Null),
            fscript_ast::LiteralPattern::Undefined { .. } => matches!(value, Value::Undefined),
        }),
        Pattern::Record { fields, .. } => {
            let Value::Record(record) = value else {
                return Ok(false);
            };

            for field in fields {
                let Some(field_value) = record.get(&field.name.name) else {
                    return Ok(false);
                };

                if let Some(pattern) = &field.pattern {
                    if !pattern_matches(pattern, field_value, bindings)? {
                        return Ok(false);
                    }
                } else if bindings
                    .insert(field.name.name.clone(), field_value.clone())
                    .is_some()
                {
                    return Err(RuntimeFailed::duplicate_binding(field.name.name.clone()));
                }
            }

            Ok(true)
        }
        Pattern::Array { items, .. } => {
            let values = match value {
                Value::Array(values) | Value::Sequence(values) => values,
                _ => return Ok(false),
            };

            if values.len() != items.len() {
                return Ok(false);
            }

            for (pattern, item_value) in items.iter().zip(values.iter()) {
                if !pattern_matches(pattern, item_value, bindings)? {
                    return Ok(false);
                }
            }

            Ok(true)
        }
    }
}

fn evaluate_expr(expr: &Expr, environment: &Environment) -> Result<Value, RuntimeFailed> {
    match evaluate_expr_with_yields(expr, environment, None) {
        Ok(EvalOutcome::Value(value)) => Ok(value),
        Ok(EvalOutcome::Throw(value)) | Err(RuntimeControl::Throw(value)) => {
            Err(RuntimeFailed::uncaught_throw(value))
        }
        Err(RuntimeControl::Error(error)) => Err(error),
    }
}

fn evaluate_expr_with_yields(
    expr: &Expr,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<EvalOutcome> {
    Ok(match expr {
        Expr::StringLiteral { value, .. } => EvalOutcome::Value(Value::String(value.clone())),
        Expr::NumberLiteral { value, .. } => EvalOutcome::Value(Value::Number(*value)),
        Expr::BooleanLiteral { value, .. } => EvalOutcome::Value(Value::Boolean(*value)),
        Expr::Null { .. } => EvalOutcome::Value(Value::Null),
        Expr::Undefined { .. } => EvalOutcome::Value(Value::Undefined),
        Expr::Identifier(identifier) => EvalOutcome::Value(
            environment
                .get(&identifier.name)
                .cloned()
                .ok_or_else(|| RuntimeFailed::unknown_identifier(identifier.name.clone()))
                .map_err(RuntimeControl::from)?,
        ),
        Expr::Record { fields, .. } => EvalOutcome::Value(Value::Record(evaluate_record_fields(
            fields,
            environment,
            yield_values,
        )?)),
        Expr::Array { items, .. } => EvalOutcome::Value(Value::Array(
            items
                .iter()
                .map(|item| evaluate_value_expr(item, environment, yield_values))
                .collect::<RuntimeEval<Vec<_>>>()?,
        )),
        Expr::Function {
            parameters,
            body,
            is_generator,
            ..
        } => EvalOutcome::Value(Value::Function(FunctionValue {
            parameters: parameters.clone(),
            body: body.clone(),
            environment: environment.clone(),
            applied_args: Vec::new(),
            is_generator: *is_generator,
        })),
        Expr::Grouped { inner, .. } => evaluate_expr_with_yields(inner, environment, yield_values)?,
        Expr::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => match consume_expr_value(condition, environment, yield_values)? {
            Value::Boolean(true) => {
                evaluate_expr_with_yields(then_branch, environment, yield_values)?
            }
            Value::Boolean(false) => {
                if let Some(else_branch) = else_branch {
                    evaluate_expr_with_yields(else_branch, environment, yield_values)?
                } else {
                    EvalOutcome::Value(Value::Undefined)
                }
            }
            other => {
                return Err(RuntimeControl::Error(RuntimeFailed::type_mismatch(
                    format!("`if` conditions must evaluate to Boolean values, found `{other}`"),
                )));
            }
        },
        Expr::Match { value, arms, .. } => {
            let value = consume_expr_value(value, environment, yield_values)?;
            evaluate_match_arms(arms, value, environment, yield_values)?
        }
        Expr::Try {
            body,
            catch_pattern,
            catch_body,
            ..
        } => match evaluate_expr_with_yields(body, environment, yield_values) {
            Ok(EvalOutcome::Value(value)) => EvalOutcome::Value(value),
            Ok(EvalOutcome::Throw(thrown)) | Err(RuntimeControl::Throw(thrown)) => {
                let mut catch_environment = environment.clone();
                bind_pattern_value(&mut catch_environment, catch_pattern, thrown, yield_values)?;
                evaluate_expr_with_yields(catch_body, &catch_environment, yield_values)?
            }
            Err(RuntimeControl::Error(error)) => return Err(RuntimeControl::Error(error)),
        },
        Expr::Throw { value, .. } => {
            EvalOutcome::Throw(evaluate_value_expr(value, environment, yield_values)?)
        }
        Expr::Binary {
            operator,
            left,
            right,
            ..
        } => {
            let left = consume_expr_value(left, environment, yield_values)?;
            let right = consume_expr_value(right, environment, yield_values)?;
            EvalOutcome::Value(
                evaluate_binary_expr(*operator, left, right).map_err(RuntimeControl::from)?,
            )
        }
        Expr::Block { items, .. } => {
            let mut block_environment = environment.clone();
            let mut last_value = Value::Undefined;

            for item in items {
                match item {
                    BlockItem::Binding(binding) => {
                        let value =
                            evaluate_value_expr(&binding.value, &block_environment, yield_values)?;
                        bind_pattern_value(
                            &mut block_environment,
                            &binding.pattern,
                            value,
                            yield_values,
                        )?;
                    }
                    BlockItem::Expr(expr) => {
                        last_value = evaluate_value_expr(expr, &block_environment, yield_values)?;
                    }
                }
            }

            EvalOutcome::Value(last_value)
        }
        Expr::Yield { value, .. } => {
            let Some(yield_values) = yield_values else {
                return Err(
                    RuntimeFailed::unsupported("`yield` outside generator execution").into(),
                );
            };
            let value = evaluate_value_expr(value, environment, Some(yield_values))?;
            yield_values.borrow_mut().push(value.clone());
            EvalOutcome::Value(value)
        }
        Expr::Unary {
            operator, operand, ..
        } => match operator {
            UnaryOperator::Defer => EvalOutcome::Value(Value::Deferred(DeferredValue {
                expr: operand.clone(),
                environment: environment.clone(),
                resolved: Rc::new(RefCell::new(None)),
            })),
            _ => {
                let operand = consume_expr_value(operand, environment, yield_values)?;
                EvalOutcome::Value(
                    evaluate_unary_expr(*operator, operand).map_err(RuntimeControl::from)?,
                )
            }
        },
        Expr::Pipe { left, right, .. } => {
            let value = evaluate_value_expr(left, environment, yield_values)?;
            evaluate_pipe_expr(right, value, environment, yield_values)?
        }
        Expr::Call { callee, args, .. } => {
            let callee = evaluate_value_expr(callee, environment, yield_values)?;
            let args = args
                .iter()
                .map(|arg| evaluate_value_expr(arg, environment, yield_values))
                .collect::<RuntimeEval<Vec<_>>>()?;
            EvalOutcome::Value(call_value(callee, args, yield_values)?)
        }
        Expr::Member {
            object, property, ..
        } => {
            let object = consume_expr_value(object, environment, yield_values)?;
            let Value::Record(fields) = object else {
                return Err(RuntimeControl::from(RuntimeFailed::type_mismatch(format!(
                    "cannot read property `{}` from non-record value",
                    property.name
                ))));
            };

            EvalOutcome::Value(
                fields
                    .get(&property.name)
                    .cloned()
                    .ok_or_else(|| RuntimeFailed::missing_property(&property.name))
                    .map_err(RuntimeControl::from)?,
            )
        }
        Expr::Index { object, index, .. } => {
            let object = consume_expr_value(object, environment, yield_values)?;
            let index = consume_expr_value(index, environment, yield_values)?;
            EvalOutcome::Value(evaluate_index_expr(object, index).map_err(RuntimeControl::from)?)
        }
    })
}

fn evaluate_record_fields(
    fields: &[fscript_ast::RecordField],
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<BTreeMap<String, Value>> {
    let mut record = BTreeMap::new();

    for field in fields {
        let value = evaluate_value_expr(&field.value, environment, yield_values)?;
        record.insert(field.name.name.clone(), value);
    }

    Ok(record)
}

fn evaluate_match_arms(
    arms: &[MatchArm],
    value: Value,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<EvalOutcome> {
    for arm in arms {
        if let Some(bindings) = match_pattern(&arm.pattern, &value).map_err(RuntimeControl::from)? {
            let mut arm_environment = environment.clone();
            for (name, binding_value) in bindings {
                define_binding(&mut arm_environment, &name, binding_value)
                    .map_err(RuntimeControl::from)?;
            }

            return evaluate_expr_with_yields(&arm.body, &arm_environment, yield_values);
        }
    }

    Err(RuntimeFailed::no_match_arm().into())
}

fn evaluate_pipe_expr(
    right: &Expr,
    piped_value: Value,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<EvalOutcome> {
    match right {
        Expr::Call { callee, args, .. } => {
            let callee = evaluate_value_expr(callee, environment, yield_values)?;
            let mut args = args
                .iter()
                .map(|arg| evaluate_value_expr(arg, environment, yield_values))
                .collect::<RuntimeEval<Vec<_>>>()?;
            args.push(piped_value);
            Ok(EvalOutcome::Value(call_value(callee, args, yield_values)?))
        }
        other => {
            let callee = evaluate_value_expr(other, environment, yield_values)?;
            Ok(EvalOutcome::Value(call_value(
                callee,
                vec![piped_value],
                yield_values,
            )?))
        }
    }
}

fn evaluate_unary_expr(operator: UnaryOperator, operand: Value) -> RuntimeResult<Value> {
    match operator {
        UnaryOperator::Not => match operand {
            Value::Boolean(value) => Ok(Value::Boolean(!value)),
            other => Err(RuntimeFailed::type_mismatch(format!(
                "cannot apply `!` to value `{other}`"
            ))),
        },
        UnaryOperator::Negate => match operand {
            Value::Number(value) => Ok(Value::Number(-value)),
            other => Err(RuntimeFailed::type_mismatch(format!(
                "cannot negate value `{other}`"
            ))),
        },
        UnaryOperator::Positive => match operand {
            Value::Number(value) => Ok(Value::Number(value)),
            other => Err(RuntimeFailed::type_mismatch(format!(
                "cannot apply unary `+` to value `{other}`"
            ))),
        },
        UnaryOperator::Defer => Err(RuntimeFailed::unsupported("`defer` expressions")),
    }
}

fn evaluate_value_expr(
    expr: &Expr,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    match evaluate_expr_with_yields(expr, environment, yield_values)? {
        EvalOutcome::Value(value) => Ok(value),
        EvalOutcome::Throw(value) => Err(RuntimeControl::Throw(value)),
    }
}

fn consume_expr_value(
    expr: &Expr,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    let value = evaluate_value_expr(expr, environment, yield_values)?;
    force_value(value, yield_values)
}

fn prepare_pattern_value(
    pattern: &Pattern,
    value: Value,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    match pattern {
        Pattern::Identifier(_) => Ok(value),
        _ => force_value(value, yield_values),
    }
}

fn force_value(value: Value, yield_values: YieldValues<'_>) -> RuntimeEval<Value> {
    match value {
        Value::Deferred(deferred) => {
            if let Some(outcome) = deferred.resolved.borrow().clone() {
                return resolve_deferred_outcome(outcome, yield_values);
            }

            let outcome =
                evaluate_expr_with_yields(&deferred.expr, &deferred.environment, yield_values)?;
            *deferred.resolved.borrow_mut() = Some(outcome.clone());
            resolve_deferred_outcome(outcome, yield_values)
        }
        other => Ok(other),
    }
}

fn resolve_deferred_outcome(
    outcome: EvalOutcome,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    match outcome {
        EvalOutcome::Value(value) => force_value(value, yield_values),
        EvalOutcome::Throw(value) => Err(RuntimeControl::Throw(value)),
    }
}

fn runtime_control_to_failed(control: RuntimeControl) -> RuntimeFailed {
    match control {
        RuntimeControl::Error(error) => error,
        RuntimeControl::Throw(value) => RuntimeFailed::uncaught_throw(value),
    }
}

fn evaluate_index_expr(object: Value, index: Value) -> Result<Value, RuntimeFailed> {
    let Value::Number(index) = index else {
        return Err(RuntimeFailed::type_mismatch(
            "array indexes must evaluate to numbers",
        ));
    };

    if index.is_sign_negative() || index.fract() != 0.0 {
        return Err(RuntimeFailed::type_mismatch(
            "array indexes must be non-negative whole numbers",
        ));
    }

    let index = index as usize;
    match object {
        Value::Array(items) | Value::Sequence(items) => {
            items.get(index).cloned().ok_or_else(|| {
                RuntimeFailed::type_mismatch(format!("index `{index}` is out of bounds"))
            })
        }
        other => Err(RuntimeFailed::type_mismatch(format!(
            "cannot index into value `{other}`"
        ))),
    }
}

fn call_value(
    callee: Value,
    args: Vec<Value>,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    let callee = force_value(callee, yield_values)?;
    let args = args
        .into_iter()
        .map(|arg| force_value(arg, yield_values))
        .collect::<RuntimeEval<Vec<_>>>()?;

    match callee {
        Value::Function(function) => call_function(function, args, yield_values),
        Value::NativeFunction(function) => call_native_function(function, args, yield_values),
        other => Err(RuntimeFailed::unsupported_call(&other).into()),
    }
}

fn call_function(
    function: FunctionValue,
    args: Vec<Value>,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    let mut all_args = function.applied_args.clone();
    all_args.extend(args);

    match all_args.len().cmp(&function.arity()) {
        std::cmp::Ordering::Less => Ok(Value::Function(function.with_args(all_args))),
        std::cmp::Ordering::Greater => Err(RuntimeFailed::type_mismatch(format!(
            "function expected {} arguments but received {}",
            function.arity(),
            all_args.len()
        ))
        .into()),
        std::cmp::Ordering::Equal => {
            let mut call_environment = function.environment.clone();
            for (parameter, argument) in function.parameters.iter().zip(all_args) {
                bind_pattern_value(
                    &mut call_environment,
                    &parameter.pattern,
                    argument,
                    yield_values,
                )?;
            }

            if function.is_generator {
                let yielded = RefCell::new(Vec::new());
                match evaluate_expr_with_yields(&function.body, &call_environment, Some(&yielded))?
                {
                    EvalOutcome::Value(_) => Ok(Value::Sequence(yielded.into_inner())),
                    EvalOutcome::Throw(value) => Err(RuntimeControl::Throw(value)),
                }
            } else {
                evaluate_value_expr(&function.body, &call_environment, yield_values)
            }
        }
    }
}

fn call_native_function(
    function: NativeFunctionValue,
    args: Vec<Value>,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    let mut all_args = function.applied_args.clone();
    all_args.extend(args);

    match all_args.len().cmp(&function.function.arity()) {
        std::cmp::Ordering::Less => Ok(Value::NativeFunction(function.with_args(all_args))),
        std::cmp::Ordering::Equal => {
            execute_native_function(function.function, all_args, yield_values)
        }
        std::cmp::Ordering::Greater => Err(RuntimeFailed::type_mismatch(format!(
            "{} expected {} arguments but received {}",
            function.function.name(),
            function.function.arity(),
            all_args.len()
        ))
        .into()),
    }
}

fn execute_native_function(
    function: NativeFunction,
    args: Vec<Value>,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    match function {
        NativeFunction::ObjectSpread => {
            let [left, right]: [Value; 2] = args.try_into().map_err(|_| {
                RuntimeFailed::type_mismatch("Object.spread expected exactly 2 arguments")
            })?;
            spread_records(left, right).map_err(RuntimeControl::from)
        }
        NativeFunction::ArrayMap => array_map(args, yield_values),
        NativeFunction::ArrayFilter => array_filter(args, yield_values),
        NativeFunction::ArrayLength => array_length(args).map_err(RuntimeControl::from),
        NativeFunction::HttpServe
        | NativeFunction::JsonToObject
        | NativeFunction::JsonToString
        | NativeFunction::JsonToPrettyString
        | NativeFunction::LoggerCreate
        | NativeFunction::LoggerLog
        | NativeFunction::LoggerDebug
        | NativeFunction::LoggerInfo
        | NativeFunction::LoggerWarn
        | NativeFunction::LoggerError
        | NativeFunction::LoggerPrettyJson
        | NativeFunction::FilesystemReadFile
        | NativeFunction::FilesystemWriteFile
        | NativeFunction::FilesystemExists
        | NativeFunction::FilesystemDeleteFile
        | NativeFunction::FilesystemReadDir
        | NativeFunction::TaskAll
        | NativeFunction::TaskRace
        | NativeFunction::TaskSpawn
        | NativeFunction::TaskDefer
        | NativeFunction::TaskForce => Err(RuntimeFailed::type_mismatch(
            "this native function is only available through the shared runtime path",
        )
        .into()),
        NativeFunction::StringTrim => {
            map_string_value(args, |value| value.trim().to_owned()).map_err(RuntimeControl::from)
        }
        NativeFunction::StringUppercase => {
            map_string_value(args, |value| value.to_uppercase()).map_err(RuntimeControl::from)
        }
        NativeFunction::StringLowercase => {
            map_string_value(args, |value| value.to_lowercase()).map_err(RuntimeControl::from)
        }
        NativeFunction::StringIsDigits => string_is_digits(args).map_err(RuntimeControl::from),
        NativeFunction::NumberParse => number_parse(args).map_err(RuntimeControl::from),
        NativeFunction::ResultOk => {
            tagged_result("ok", "value", args).map_err(RuntimeControl::from)
        }
        NativeFunction::ResultError => {
            tagged_result("error", "error", args).map_err(RuntimeControl::from)
        }
        NativeFunction::ResultIsOk => result_has_tag("ok", args).map_err(RuntimeControl::from),
        NativeFunction::ResultIsError => {
            result_has_tag("error", args).map_err(RuntimeControl::from)
        }
        NativeFunction::ResultWithDefault => {
            result_with_default(args).map_err(RuntimeControl::from)
        }
    }
}

fn spread_records(left: Value, right: Value) -> Result<Value, RuntimeFailed> {
    let Value::Record(left) = left else {
        return Err(RuntimeFailed::type_mismatch(
            "Object.spread expects record values for its left argument",
        ));
    };
    let Value::Record(right) = right else {
        return Err(RuntimeFailed::type_mismatch(
            "Object.spread expects record values for its right argument",
        ));
    };

    let mut merged = left;
    for (key, value) in right {
        merged.insert(key, value);
    }

    Ok(Value::Record(merged))
}

fn array_map(args: Vec<Value>, yield_values: YieldValues<'_>) -> RuntimeEval<Value> {
    let [function, items]: [Value; 2] = args
        .try_into()
        .map_err(|_| RuntimeFailed::type_mismatch("Array.map expected exactly 2 arguments"))?;
    let Value::Array(items) = items else {
        return Err(RuntimeControl::Error(RuntimeFailed::type_mismatch(
            "Array.map expects an array as its final argument",
        )));
    };

    let mapped = items
        .into_iter()
        .map(|item| call_value(function.clone(), vec![item], yield_values))
        .collect::<RuntimeEval<Vec<_>>>()?;

    Ok(Value::Array(mapped))
}

fn array_filter(args: Vec<Value>, yield_values: YieldValues<'_>) -> RuntimeEval<Value> {
    let [function, items]: [Value; 2] = args
        .try_into()
        .map_err(|_| RuntimeFailed::type_mismatch("Array.filter expected exactly 2 arguments"))?;
    let Value::Array(items) = items else {
        return Err(RuntimeControl::Error(RuntimeFailed::type_mismatch(
            "Array.filter expects an array as its final argument",
        )));
    };

    let mut filtered = Vec::new();
    for item in items {
        match call_value(function.clone(), vec![item.clone()], yield_values)? {
            Value::Boolean(true) => filtered.push(item),
            Value::Boolean(false) => {}
            other => {
                return Err(RuntimeControl::from(RuntimeFailed::type_mismatch(format!(
                    "Array.filter callbacks must return Boolean values, found `{other}`"
                ))));
            }
        }
    }

    Ok(Value::Array(filtered))
}

fn array_length(args: Vec<Value>) -> Result<Value, RuntimeFailed> {
    let [items]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeFailed::type_mismatch("Array.length expected exactly 1 argument"))?;

    match items {
        Value::Array(items) => Ok(Value::Number(items.len() as f64)),
        Value::Sequence(items) => Ok(Value::Number(items.len() as f64)),
        other => Err(RuntimeFailed::type_mismatch(format!(
            "Array.length expects an array value, found `{other}`"
        ))),
    }
}

fn map_string_value(
    args: Vec<Value>,
    map: impl FnOnce(&str) -> String,
) -> Result<Value, RuntimeFailed> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeFailed::type_mismatch("string helpers expect exactly 1 argument"))?;
    let Value::String(value) = value else {
        return Err(RuntimeFailed::type_mismatch(
            "string helpers expect a String argument",
        ));
    };

    Ok(Value::String(map(&value)))
}

fn string_is_digits(args: Vec<Value>) -> Result<Value, RuntimeFailed> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeFailed::type_mismatch("String.isDigits expected exactly 1 argument"))?;
    let Value::String(value) = value else {
        return Err(RuntimeFailed::type_mismatch(
            "String.isDigits expects a String argument",
        ));
    };

    Ok(Value::Boolean(
        !value.is_empty() && value.chars().all(|character| character.is_ascii_digit()),
    ))
}

fn number_parse(args: Vec<Value>) -> Result<Value, RuntimeFailed> {
    let [value]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeFailed::type_mismatch("Number.parse expected exactly 1 argument"))?;
    let Value::String(value) = value else {
        return Err(RuntimeFailed::type_mismatch(
            "Number.parse expects a String argument",
        ));
    };

    value
        .parse::<f64>()
        .map(Value::Number)
        .map_err(|_| RuntimeFailed::type_mismatch(format!("could not parse `{value}` as a Number")))
}

fn tagged_result(tag: &str, field: &str, args: Vec<Value>) -> Result<Value, RuntimeFailed> {
    let [value]: [Value; 1] = args.try_into().map_err(|_| {
        RuntimeFailed::type_mismatch("Result constructors expect exactly 1 argument")
    })?;

    Ok(Value::Record(BTreeMap::from([
        ("tag".to_owned(), Value::String(tag.to_owned())),
        (field.to_owned(), value),
    ])))
}

fn result_has_tag(expected_tag: &str, args: Vec<Value>) -> Result<Value, RuntimeFailed> {
    let [result]: [Value; 1] = args
        .try_into()
        .map_err(|_| RuntimeFailed::type_mismatch("Result predicates expect exactly 1 argument"))?;
    let Value::Record(fields) = result else {
        return Err(RuntimeFailed::type_mismatch(
            "Result predicates expect a Result record",
        ));
    };

    Ok(Value::Boolean(
        fields.get("tag") == Some(&Value::String(expected_tag.to_owned())),
    ))
}

fn result_with_default(args: Vec<Value>) -> Result<Value, RuntimeFailed> {
    let [fallback, result]: [Value; 2] = args.try_into().map_err(|_| {
        RuntimeFailed::type_mismatch("Result.withDefault expected exactly 2 arguments")
    })?;
    let Value::Record(fields) = result else {
        return Err(RuntimeFailed::type_mismatch(
            "Result.withDefault expects a Result record",
        ));
    };

    match fields.get("tag") {
        Some(Value::String(tag)) if tag == "ok" => fields
            .get("value")
            .cloned()
            .ok_or_else(|| RuntimeFailed::type_mismatch("ok results must contain a `value` field")),
        Some(Value::String(tag)) if tag == "error" => Ok(fallback),
        _ => Err(RuntimeFailed::type_mismatch(
            "Result.withDefault expects a tagged Result record",
        )),
    }
}

fn evaluate_binary_expr(
    operator: BinaryOperator,
    left: Value,
    right: Value,
) -> Result<Value, RuntimeFailed> {
    match operator {
        BinaryOperator::Add => match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left + right)),
            (Value::String(left), Value::String(right)) => Ok(Value::String(left + &right)),
            (Value::String(left), Value::Number(right)) => {
                Ok(Value::String(format!("{left}{right}")))
            }
            (Value::Number(left), Value::String(right)) => {
                Ok(Value::String(format!("{left}{right}")))
            }
            (left, right) => Err(RuntimeFailed::type_mismatch(format!(
                "cannot add values `{left}` and `{right}`"
            ))),
        },
        BinaryOperator::Subtract => match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left - right)),
            (left, right) => Err(RuntimeFailed::type_mismatch(format!(
                "cannot subtract values `{left}` and `{right}`"
            ))),
        },
        BinaryOperator::Multiply => match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left * right)),
            (left, right) => Err(RuntimeFailed::type_mismatch(format!(
                "cannot multiply values `{left}` and `{right}`"
            ))),
        },
        BinaryOperator::Divide => match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left / right)),
            (left, right) => Err(RuntimeFailed::type_mismatch(format!(
                "cannot divide values `{left}` and `{right}`"
            ))),
        },
        BinaryOperator::Modulo => match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left % right)),
            (left, right) => Err(RuntimeFailed::type_mismatch(format!(
                "cannot apply `%` to values `{left}` and `{right}`"
            ))),
        },
        BinaryOperator::StrictEqual => {
            Ok(Value::Boolean(compare_structural_equality(&left, &right)?))
        }
        BinaryOperator::StrictNotEqual => {
            Ok(Value::Boolean(!compare_structural_equality(&left, &right)?))
        }
        BinaryOperator::Less => compare_numbers(left, right, |left, right| left < right),
        BinaryOperator::LessEqual => compare_numbers(left, right, |left, right| left <= right),
        BinaryOperator::Greater => compare_numbers(left, right, |left, right| left > right),
        BinaryOperator::GreaterEqual => compare_numbers(left, right, |left, right| left >= right),
        BinaryOperator::LogicalOr => compare_booleans(left, right, |left, right| left || right),
        BinaryOperator::LogicalAnd => compare_booleans(left, right, |left, right| left && right),
        BinaryOperator::NullishCoalesce => Ok(match left {
            Value::Null | Value::Undefined => right,
            value => value,
        }),
    }
}

fn compare_numbers(
    left: Value,
    right: Value,
    compare: impl FnOnce(f64, f64) -> bool,
) -> Result<Value, RuntimeFailed> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => Ok(Value::Boolean(compare(left, right))),
        (left, right) => Err(RuntimeFailed::type_mismatch(format!(
            "expected numbers but found `{left}` and `{right}`"
        ))),
    }
}

fn compare_booleans(
    left: Value,
    right: Value,
    combine: impl FnOnce(bool, bool) -> bool,
) -> Result<Value, RuntimeFailed> {
    match (left, right) {
        (Value::Boolean(left), Value::Boolean(right)) => Ok(Value::Boolean(combine(left, right))),
        (left, right) => Err(RuntimeFailed::type_mismatch(format!(
            "expected booleans but found `{left}` and `{right}`"
        ))),
    }
}

fn compare_structural_equality(left: &Value, right: &Value) -> Result<bool, RuntimeFailed> {
    match (left, right) {
        (Value::Function(_), _)
        | (_, Value::Function(_))
        | (Value::NativeFunction(_), _)
        | (_, Value::NativeFunction(_)) => Err(RuntimeFailed::type_mismatch(
            "functions cannot be compared with `===` or `!==`",
        )),
        _ => Ok(left == right),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::rc::Rc;
    use std::thread;
    use std::time::{Duration, Instant};

    use camino::{Utf8Path, Utf8PathBuf};
    use fscript_ast::{
        Expr as AstExpr, Identifier, ImportClause, ImportDecl, LiteralPattern, Parameter, Pattern,
        RecordPatternField, UnaryOperator,
    };
    use fscript_codegen_cranelift::CompileError;
    use fscript_lexer::{LexDiagnostic, LexDiagnosticKind, TokenKind};
    use fscript_parser::{ParseDiagnostic, ParseDiagnosticKind};
    use fscript_source::{SourceFile, Span};
    use fscript_test_support::{
        canonicalize_utf8, example_source_paths, normalize_snapshot,
        write_temp_file as support_write_temp_file,
        write_temp_project as support_write_temp_project,
    };
    use insta::assert_snapshot;
    use miette::{Diagnostic, SourceSpan};

    use super::{
        CompileFailed, CompileSourceFailed, DeferredValue, DiagnosticSummary, DriverError,
        Environment, EvalOutcome, FunctionValue, LexFailed, LoadedProgram, NativeFunction,
        NativeFunctionValue, ParseFailed, RuntimeFailed, Value, call_native_function, check_file,
        check_source, collect_after_lines, collect_before_lines, compile_file, define_binding,
        evaluate_index_expr, evaluate_unary_expr, highlight_source_line, load_import,
        load_module_from_source, match_pattern, reject_non_std_imports, render_source_error,
        resolve_import_path, run_file, run_modules, run_source, runtime_control_to_failed,
    };

    fn span() -> Span {
        Span::new(0, 0)
    }

    fn identifier(name: &str) -> Identifier {
        Identifier {
            name: name.to_owned(),
            span: span(),
        }
    }

    fn identifier_pattern(name: &str) -> Pattern {
        Pattern::Identifier(identifier(name))
    }

    fn number(value: f64) -> AstExpr {
        AstExpr::NumberLiteral {
            value,
            span: span(),
        }
    }

    fn function_with_parameter_pattern(pattern: Pattern) -> Value {
        Value::Function(FunctionValue {
            parameters: vec![Parameter {
                pattern,
                type_annotation: None,
                span: span(),
            }],
            body: Box::new(number(1.0)),
            environment: Environment::new(),
            applied_args: Vec::new(),
            is_generator: false,
        })
    }

    fn write_temp_file(name: &str, contents: &str) -> Utf8PathBuf {
        support_write_temp_file(&format!("driver-{name}"), contents)
    }

    fn write_temp_project(name: &str, files: &[(&str, &str)]) -> Utf8PathBuf {
        support_write_temp_project(&format!("driver-{name}"), files)
    }

    fn temp_output_path(name: &str) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(
            std::env::temp_dir().join(format!("fscript-driver-{name}-{}", std::process::id())),
        )
        .expect("temp paths are utf-8 in tests")
    }

    fn compiled_stdout(source_path: &Utf8Path, name: &str) -> String {
        let output_path = temp_output_path(name);
        compile_file(source_path, &output_path).expect("supported sources should compile");

        let output = std::process::Command::new(&output_path)
            .output()
            .expect("compiled binary should run");

        let _ = fs::remove_file(&output_path);

        assert!(output.status.success());
        String::from_utf8_lossy(&output.stdout).into_owned()
    }

    fn canonicalize(path: &Utf8Path) -> Utf8PathBuf {
        Utf8PathBuf::from_path_buf(
            fs::canonicalize(path).expect("test paths should be canonicalizable"),
        )
        .expect("test paths should stay utf-8")
    }

    fn unused_port() -> u16 {
        TcpListener::bind(("127.0.0.1", 0))
            .expect("an ephemeral port should be available")
            .local_addr()
            .expect("the listener should have a local address")
            .port()
    }

    fn all_example_paths() -> Vec<Utf8PathBuf> {
        example_source_paths()
            .into_iter()
            .filter(|path| path.parent() == Some(fscript_test_support::examples_dir().as_path()))
            .collect()
    }

    #[test]
    fn check_file_succeeds_for_lexically_valid_source() {
        let path = write_temp_file("valid", "answer = 42");

        let summary = check_file(&path).expect("valid source should lex successfully");

        assert_eq!(summary.path, canonicalize_utf8(&path));
        assert!(summary.token_count >= 3);
    }

    #[test]
    fn check_source_succeeds_for_in_memory_programs() {
        let summary = check_source("answer = 42").expect("valid in-memory source should check");

        assert_eq!(summary.path, Utf8PathBuf::from("sandbox.fs"));
        assert!(summary.token_count >= 3);
    }

    #[test]
    fn check_file_rejects_parse_errors() {
        let path = write_temp_file("parse-error", "person = 'Ada'broken");

        assert!(matches!(check_file(&path), Err(DriverError::Parse(_))));
    }

    #[test]
    fn check_file_rejects_if_expressions_without_else() {
        let path = write_temp_file("if-missing-else", "answer = if (true) { 1 }");

        assert!(matches!(check_file(&path), Err(DriverError::Parse(_))));
    }

    #[test]
    fn check_file_rejects_type_errors() {
        let path = write_temp_file(
            "type-error",
            "greet = (name: String): Number => 'hello, ' + name",
        );

        assert!(matches!(check_file(&path), Err(DriverError::Type(_))));
    }

    #[test]
    fn check_file_rejects_non_exhaustive_tagged_union_matches() {
        let path = write_temp_file(
            "non-exhaustive-match",
            "type User =\n\
             | { tag: 'guest' }\n\
             | { tag: 'member', name: String }\n\
             \n\
             describe = (user: User): String => match (user) {\n\
               { tag: 'guest' } => 'Guest',\n\
             }",
        );

        assert!(matches!(check_file(&path), Err(DriverError::Type(_))));
    }

    #[test]
    fn check_file_rejects_effectful_generator_work() {
        let path = write_temp_file(
            "effect-error",
            "import Filesystem from 'std:filesystem'\n\
             \n\
             load_lines = *(path: String) => {\n\
               yield Filesystem.readFile(path)\n\
             }",
        );

        assert!(matches!(check_file(&path), Err(DriverError::Effect(_))));
    }

    #[test]
    fn check_file_rejects_non_fs_extensions() {
        let path = Utf8PathBuf::from("example.txt");

        let error = check_file(&path).expect_err("non-fscript extensions should fail");

        assert!(matches!(error, DriverError::UnsupportedExtension(_)));
    }

    #[test]
    fn check_file_accepts_try_throw_and_defer_expressions() {
        let path = write_temp_file(
            "try-throw-defer",
            "answer = try {\n  value = defer (40 + 2)\n  value + 1\n} catch (error) {\n  error\n}",
        );

        let summary = check_file(&path).expect("the semantic frontend should accept this slice");

        assert_eq!(summary.path, canonicalize_utf8(&path));
    }

    #[test]
    fn check_file_accepts_std_task_programs() {
        let path = write_temp_file(
            "task-check",
            "import Task from 'std:task'\n\
             task = [(): Number => 41 + 1][0]\n\
             lazyValue = Task.defer(task)\n\
             started = Task.spawn(task)\n\
             backup = Task.spawn((): Number => 1 + 1)\n\
             winner = Task.race([started, backup])\n\
             answer = Task.force(lazyValue) + winner",
        );

        let summary = check_file(&path).expect("std:task programs should typecheck");

        assert_eq!(summary.path, canonicalize_utf8(&path));
    }

    #[test]
    fn check_file_accepts_std_http_programs() {
        let path = write_temp_file(
            "http-check",
            "import Http from 'std:http'\n\
             \n\
             handler = (request: { body: String, method: String, path: String }): { body: String, contentType: String, status: Number } => {\n\
               { body: 'hello', contentType: 'text/plain', status: 200 }\n\
             }\n\
             \n\
             server = Http.serve({ host: '127.0.0.1', port: 8080, maxRequests: 1 }, handler)",
        );

        let summary = check_file(&path).expect("std:http programs should typecheck");

        assert_eq!(summary.path, canonicalize_utf8(&path));
    }

    #[test]
    fn check_file_accepts_http_hello_server_example() {
        let summary = check_file(&fscript_test_support::example_path(
            "http_hello_server/main.fs",
        ))
        .expect("the http hello server example should typecheck");

        assert_eq!(
            summary.path,
            canonicalize_utf8(&fscript_test_support::example_path(
                "http_hello_server/main.fs"
            ))
        );
    }

    #[test]
    fn check_file_rejects_circular_user_imports() {
        let project = write_temp_project(
            "cycle",
            &[
                (
                    "main.fs",
                    "import { value } from './other.fs'\nanswer = value",
                ),
                (
                    "other.fs",
                    "import { answer } from './main.fs'\nexport value = answer",
                ),
            ],
        );

        let error = check_file(&project.join("main.fs")).expect_err("cycles should fail");

        assert!(matches!(error, DriverError::Import(_)));
    }

    #[test]
    fn run_file_executes_the_first_supported_subset() {
        let path = write_temp_file("run", "person = 'world'\nmessage = 'hello, ' + person");

        let summary = run_file(&path).expect("the initial execution subset should run");

        assert_eq!(
            summary.last_value,
            Some(Value::String("hello, world".to_owned()))
        );
    }

    #[test]
    fn run_file_reports_runtime_identifier_errors() {
        let path = write_temp_file("missing", "copy = message");

        assert!(matches!(run_file(&path), Err(DriverError::Lower(_))));
    }

    #[test]
    fn run_file_supports_record_member_access() {
        let path = write_temp_file("member-access", "user = { name: 'Ada' }\nname = user.name");

        let summary = run_file(&path).expect("record member access should evaluate");

        assert_eq!(summary.last_value, Some(Value::String("Ada".to_owned())));
    }

    #[test]
    fn run_file_supports_relative_user_module_imports() {
        let project = write_temp_project(
            "imports",
            &[
                (
                    "main.fs",
                    "import { greet } from './greeter.fs'\nmessage = greet('Ada')",
                ),
                (
                    "greeter.fs",
                    "export greet = (name: String): String => 'hello, ' + name",
                ),
            ],
        );

        let summary =
            run_file(&project.join("main.fs")).expect("relative user imports should execute");

        assert_eq!(
            summary.last_value,
            Some(Value::String("hello, Ada".to_owned()))
        );
    }

    #[test]
    fn run_file_supports_std_json_and_filesystem_modules() {
        let project = write_temp_project(
            "json-filesystem",
            &[("main.fs", ""), ("user.json", "{\"name\":\"Ada\"}")],
        );
        let source = format!(
            "import Json from 'std:json'\n\
             import FileSystem from 'std:filesystem'\n\
             \n\
             data = Json.jsonToObject(FileSystem.readFile('{}'))\n\
             name = data.name",
            project.join("user.json")
        );
        fs::write(project.join("main.fs"), source).expect("main source should be writable");

        let summary =
            run_file(&project.join("main.fs")).expect("json and filesystem std modules should run");

        assert_eq!(summary.last_value, Some(Value::String("Ada".to_owned())));
    }

    #[test]
    fn run_file_supports_std_object_spread() {
        let path = write_temp_file(
            "object-spread",
            "import Object from 'std:object'\n\
             \n\
             baseUser = {\n\
               id: 'user-1',\n\
               name: 'Ada',\n\
             }\n\
             \n\
             activeUser = Object.spread(baseUser, {\n\
               active: true,\n\
               role: 'admin',\n\
             })",
        );

        let summary = run_file(&path).expect("Object.spread should execute");

        assert_eq!(
            summary.last_value,
            Some(Value::Record(BTreeMap::from([
                ("active".to_owned(), Value::Boolean(true)),
                ("id".to_owned(), Value::String("user-1".to_owned())),
                ("name".to_owned(), Value::String("Ada".to_owned())),
                ("role".to_owned(), Value::String("admin".to_owned())),
            ])))
        );
    }

    #[test]
    fn run_file_supports_curried_native_std_functions() {
        let path = write_temp_file(
            "object-spread-curried",
            "import Object from 'std:object'\n\
             \n\
             mergeActive = Object.spread({ active: true })\n\
             user = mergeActive({ name: 'Ada' })",
        );

        let summary = run_file(&path).expect("native std functions should support currying");

        assert_eq!(
            summary.last_value,
            Some(Value::Record(BTreeMap::from([
                ("active".to_owned(), Value::Boolean(true)),
                ("name".to_owned(), Value::String("Ada".to_owned())),
            ])))
        );
    }

    #[test]
    fn run_file_supports_try_catch_throw() {
        let path = write_temp_file(
            "try-catch-throw",
            "answer = try {\n  throw { tag: 'boom', message: 'recovered' }\n} catch ({ message }) {\n  message\n}",
        );

        let summary = run_file(&path).expect("try/catch should recover thrown values");

        assert_eq!(
            summary.last_value,
            Some(Value::String("recovered".to_owned()))
        );
    }

    #[test]
    fn run_file_forces_deferred_values_when_consumed() {
        let path = write_temp_file(
            "defer-consume",
            "increment = (value: Number): Number => value + 1\nanswer = increment(defer 41)",
        );

        let summary = run_file(&path).expect("calls should force deferred arguments");

        assert_eq!(summary.last_value, Some(Value::Number(42.0)));
    }

    #[test]
    fn run_file_supports_std_task_defer_and_force() {
        let path = write_temp_file(
            "task-force",
            "import Task from 'std:task'\n\
             task = [(): Number => 41 + 1][0]\n\
             lazyValue = Task.defer(task)\n\
             answer = Task.force(lazyValue)",
        );

        let summary = run_file(&path).expect("Task.defer and Task.force should execute");

        assert_eq!(summary.last_value, Some(Value::Number(42.0)));
    }

    #[test]
    fn run_file_supports_std_task_all_batches_zero_arg_work() {
        let path = write_temp_file(
            "task-all",
            "import Task from 'std:task'\n\
             tasks = [\n\
               Task.spawn((): Number => 1),\n\
               Task.spawn((): Number => 2),\n\
               Task.spawn((): Number => 3),\n\
             ]\n\
             answers = Task.all(tasks)\n\
             total = answers[0] + answers[1] + answers[2]",
        );

        let summary = run_file(&path).expect("Task.all should batch deferred work");

        assert_eq!(summary.last_value, Some(Value::Number(6.0)));
    }

    #[test]
    fn run_file_supports_std_task_spawn_and_race() {
        let path = write_temp_file(
            "task-spawn-race",
            "import Task from 'std:task'\n\
             slow = (): Number => 40 + 2\n\
             fast = (): Number => 1 + 1\n\
             started = Task.spawn(slow)\n\
             fallback = Task.spawn(fast)\n\
             winner = Task.race([started, fallback])\n\
             answer = winner",
        );

        let summary = run_file(&path).expect("Task.spawn and Task.race should execute");

        assert_eq!(summary.last_value, Some(Value::Number(42.0)));
    }

    #[test]
    fn run_file_reports_uncaught_throw_values() {
        let path = write_temp_file("uncaught-throw", "answer = throw { message: 'boom' }");

        assert!(matches!(run_file(&path), Err(DriverError::Runtime(_))));
    }

    #[test]
    fn run_file_serves_http_requests_via_std_http() {
        let port = unused_port();
        let source = format!(
            "import Http from 'std:http'\n\
             \n\
             handler = (request: {{ body: String, method: String, path: String }}): {{ body: String, contentType: String, status: Number }} => {{\n\
               if (request.path === '/') {{\n\
                 {{ body: 'hello from fscript', contentType: 'text/plain', status: 200 }}\n\
               }} else {{\n\
                 {{ body: 'not found', contentType: 'text/plain', status: 404 }}\n\
               }}\n\
             }}\n\
             \n\
             server = Http.serve({{ host: '127.0.0.1', port: {port}, maxRequests: 1 }}, handler)"
        );
        let path = write_temp_file("http-run", &source);

        let client = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut last_error = None;

            while Instant::now() < deadline {
                match TcpStream::connect(("127.0.0.1", port)) {
                    Ok(mut stream) => {
                        stream
                            .write_all(
                                b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
                            )
                            .expect("the request should write");
                        let mut response = String::new();
                        stream
                            .read_to_string(&mut response)
                            .expect("the response should be readable");
                        return response;
                    }
                    Err(error) => last_error = Some(error),
                }

                thread::sleep(Duration::from_millis(25));
            }

            panic!(
                "the test server never started listening on port {port}: last connection error: {}",
                last_error
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "unknown connection failure".to_owned())
            );
        });

        let summary = run_file(&path).expect("std:http server should execute");
        let response = client.join().expect("the client thread should succeed");

        assert_eq!(summary.last_value, Some(Value::Undefined));
        assert!(response.starts_with("HTTP/1.1 200 OK"));
        assert!(response.ends_with("hello from fscript"));
    }

    #[test]
    fn run_file_executes_every_example() {
        for path in all_example_paths() {
            run_file(&path).unwrap_or_else(|error| {
                panic!("example `{path}` should execute successfully: {error}")
            });
        }
    }

    #[test]
    fn run_file_executes_array_pipeline_example() {
        let summary = run_file(&fscript_test_support::example_path("array_pipeline.fs"))
            .expect("array pipeline example should execute");

        assert_eq!(
            summary.last_value,
            Some(Value::Array(vec![
                Value::Number(4.0),
                Value::Number(5.0),
                Value::Number(6.0),
            ]))
        );
    }

    #[test]
    fn run_file_executes_hello_world_example() {
        let summary = run_file(&fscript_test_support::example_path("hello_world.fs"))
            .expect("hello world example should execute");

        assert_eq!(
            summary.last_value,
            Some(Value::String("hello, fscript user".to_owned()))
        );
    }

    #[test]
    fn run_file_executes_match_tagged_union_example() {
        let summary = run_file(&fscript_test_support::example_path("match_tagged_union.fs"))
            .expect("match example should execute");

        assert_eq!(summary.last_value, Some(Value::String("Grace".to_owned())));
    }

    #[test]
    fn run_file_executes_object_merge_example() {
        let summary = run_file(&fscript_test_support::example_path("object_merge.fs"))
            .expect("object merge example should execute");

        assert_eq!(
            summary.last_value,
            Some(Value::Record(BTreeMap::from([
                ("active".to_owned(), Value::Boolean(true)),
                ("id".to_owned(), Value::String("user-1".to_owned())),
                ("name".to_owned(), Value::String("Ada".to_owned())),
                ("role".to_owned(), Value::String("admin".to_owned())),
            ])))
        );
    }

    #[test]
    fn run_file_executes_result_error_handling_example() {
        let summary = run_file(&fscript_test_support::example_path(
            "result_error_handling.fs",
        ))
        .expect("result example should execute");

        assert_eq!(
            summary.last_value,
            Some(Value::Record(BTreeMap::from([
                ("tag".to_owned(), Value::String("ok".to_owned())),
                ("value".to_owned(), Value::Number(8080.0)),
            ])))
        );
    }

    #[test]
    fn run_file_executes_generator_counter_example() {
        let summary = run_file(&fscript_test_support::example_path("generator_counter.fs"))
            .expect("generator example should execute");

        assert_eq!(
            summary.last_value,
            Some(Value::Sequence(vec![
                Value::Number(3.0),
                Value::Number(4.0),
                Value::Number(5.0),
            ]))
        );
    }

    #[test]
    fn compile_file_emits_a_native_executable_for_the_current_subset() {
        let source_path =
            write_temp_file("compile", "person = 'world'\nmessage = 'hello, ' + person");
        assert_eq!(
            compiled_stdout(&source_path, "compiled-smoke"),
            "hello, world\n"
        );

        let _ = fs::remove_file(&source_path);
    }

    #[test]
    fn compile_file_matches_run_output_for_the_supported_subset() {
        let source_path = write_temp_file(
            "compile-parity",
            "left = 20\n\
             right = 22\n\
             answer = {\n\
               total = left + right\n\
               total\n\
             }",
        );

        let run_summary = run_file(&source_path).expect("supported source should run");
        let compiled_output = compiled_stdout(&source_path, "compiled-parity");

        assert_eq!(
            compiled_output,
            format!(
                "{}\n",
                run_summary
                    .last_value
                    .expect("the supported parity source should produce a final value")
            )
        );

        let _ = fs::remove_file(&source_path);
    }

    #[test]
    fn compile_file_matches_run_output_for_plain_data_control_flow_subset() {
        let source_path = write_temp_file(
            "compile-plain-data-parity",
            "user = { name: 'Ada', active: true }\n\
             values = [10, 20, 30]\n\
             answer = if (!false) {\n\
               user.name\n\
             } else {\n\
               values[1]\n\
             }",
        );

        let run_summary = run_file(&source_path).expect("plain-data source should run");
        let compiled_output = compiled_stdout(&source_path, "compiled-plain-data-parity");

        assert_eq!(
            compiled_output,
            format!(
                "{}\n",
                run_summary
                    .last_value
                    .expect("the plain-data parity source should produce a final value")
            )
        );

        let _ = fs::remove_file(&source_path);
    }

    #[test]
    fn compile_file_matches_run_output_for_user_module_imports() {
        let project = write_temp_project(
            "compile-import-parity",
            &[
                (
                    "main.fs",
                    "import { greet } from './greeter.fs'\nmessage = greet('Ada')",
                ),
                (
                    "greeter.fs",
                    "export greet = (name: String): String => 'hello, ' + name",
                ),
            ],
        );

        let source_path = project.join("main.fs");
        let run_summary = run_file(&source_path).expect("imported program should run");
        let compiled_output = compiled_stdout(&source_path, "compiled-import-parity");

        assert_eq!(
            compiled_output,
            format!(
                "{}\n",
                run_summary
                    .last_value
                    .expect("the imported parity source should produce a final value")
            )
        );
    }

    #[test]
    fn compile_file_matches_run_output_for_defer_and_task_programs() {
        let source_path = write_temp_file(
            "compile-task-parity",
            "import Task from 'std:task'\n\
             compute = (): Number => 40 + 2\n\
             lazyValue = Task.defer(compute)\n\
             started = Task.spawn(compute)\n\
             answer = Task.force(lazyValue) + Task.race([started, Task.spawn((): Number => 1)])",
        );

        let run_summary = run_file(&source_path).expect("task source should run");
        let compiled_output = compiled_stdout(&source_path, "compiled-task-parity");

        assert_eq!(
            compiled_output,
            format!(
                "{}\n",
                run_summary
                    .last_value
                    .expect("the task parity source should produce a final value")
            )
        );

        let _ = fs::remove_file(&source_path);
    }

    #[test]
    fn compile_file_reports_tool_failures_without_source_spans() {
        let path = write_temp_file("compile-tool-failure", "answer = 42");
        let output_directory = write_temp_project("compile-output-directory", &[]);
        let error = compile_file(&path, &output_directory)
            .expect_err("copying a binary onto a directory path should fail");

        assert!(matches!(error, DriverError::Compile(_)));
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn run_source_executes_in_memory_programs() {
        let summary = run_source("left = 1\nright = 2\nanswer = left + right")
            .expect("in-memory source should execute");

        assert_eq!(summary.path, Utf8PathBuf::from("sandbox.fs"));
        assert_eq!(summary.last_value, Some(Value::Number(3.0)));
    }

    #[test]
    fn run_source_rejects_non_std_imports() {
        let error = run_source("import Foo from './other.fs'\nanswer = Foo")
            .expect_err("import should fail");

        assert!(matches!(error, DriverError::Import(_)));
    }

    #[test]
    fn diagnostic_summary_reports_source_related_failures() {
        let missing_path = Utf8PathBuf::from("this-file-should-not-exist.fs");
        let source_summary = check_file(&missing_path)
            .expect_err("missing files should fail")
            .diagnostic_summary();
        assert_eq!(source_summary.kind, "source");
        assert!(
            source_summary
                .message
                .contains("this-file-should-not-exist.fs")
        );
        assert_eq!(source_summary.line, None);

        let extension_summary = check_file(Utf8Path::new("not-fscript.txt"))
            .expect_err("wrong extensions should fail")
            .diagnostic_summary();
        assert_eq!(extension_summary.kind, "source");
        assert_eq!(
            extension_summary.location,
            Some("not-fscript.txt".to_owned())
        );
        assert_eq!(extension_summary.label, None);

        let import_summary = run_source("import Foo from './other.fs'\nanswer = Foo")
            .expect_err("non-std imports should fail")
            .diagnostic_summary();
        assert_eq!(import_summary.kind, "import");
        assert!(import_summary.message.contains("./other.fs"));
    }

    #[test]
    fn diagnostic_summary_reports_frontend_failures() {
        let lex_path = write_temp_file("diagnostic-lex", "answer = @");
        let lex_summary = check_file(&lex_path)
            .expect_err("invalid tokens should fail")
            .diagnostic_summary();
        assert_eq!(
            lex_summary,
            DiagnosticSummary {
                kind: "lex",
                title: "invalid token `@`".to_owned(),
                message: format!(
                    "invalid token `@` at {}",
                    lex_summary
                        .location
                        .clone()
                        .expect("lex diagnostics include a location")
                ),
                line: Some(1),
                column: Some(10),
                width: Some(1),
                location: lex_summary.location.clone(),
                label: Some("this character is not valid in FScript".to_owned()),
            }
        );
        assert!(
            check_file(&lex_path)
                .expect_err("invalid tokens should fail")
                .render_pretty()
                .contains("problem starts here")
        );

        let parse_path = write_temp_file("diagnostic-parse", "answer = if (true) { 1 }");
        let parse_summary = check_file(&parse_path)
            .expect_err("parse errors should fail")
            .diagnostic_summary();
        assert_eq!(parse_summary.kind, "parse");
        assert!(parse_summary.message.contains("expected `else`"));
        assert_eq!(parse_summary.line, Some(1));

        let lower_path = write_temp_file("diagnostic-lower", "answer = missing");
        let lower_summary = run_file(&lower_path)
            .expect_err("lowering failures should fail")
            .diagnostic_summary();
        assert_eq!(lower_summary.kind, "lower");
        assert!(
            lower_summary
                .message
                .contains("unknown identifier `missing`")
        );

        let type_path = write_temp_file(
            "diagnostic-type",
            "greet = (name: String): Number => 'hello, ' + name",
        );
        let type_summary = check_file(&type_path)
            .expect_err("type errors should fail")
            .diagnostic_summary();
        assert_eq!(type_summary.kind, "type");
        assert!(type_summary.message.contains("Number"));

        let effect_path = write_temp_file(
            "diagnostic-effect",
            "import Filesystem from 'std:filesystem'\n\
             load_lines = *(path: String) => {\n\
               yield Filesystem.readFile(path)\n\
             }",
        );
        let effect_summary = check_file(&effect_path)
            .expect_err("effect errors should fail")
            .diagnostic_summary();
        assert_eq!(effect_summary.kind, "effect");
        assert!(effect_summary.message.contains("generator"));
    }

    #[test]
    fn diagnostic_summary_reports_runtime_and_compile_tool_failures() {
        let runtime_summary = run_file(&write_temp_file(
            "diagnostic-runtime",
            "answer = throw { message: 'boom' }",
        ))
        .expect_err("runtime failures should fail")
        .diagnostic_summary();
        assert_eq!(runtime_summary.kind, "runtime");
        assert_eq!(
            runtime_summary.message,
            "uncaught thrown value `{ message: 'boom' }`"
        );
        assert_eq!(runtime_summary.line, None);

        let compile_path = write_temp_file("diagnostic-compile-tool", "answer = 42");
        let output_directory = write_temp_project("diagnostic-compile-output-directory", &[]);
        let compile_summary = compile_file(&compile_path, &output_directory)
            .expect_err("tool failures should fail")
            .diagnostic_summary();
        assert_eq!(compile_summary.kind, "compile");
        assert!(!compile_summary.message.is_empty());
        assert_eq!(compile_summary.location, None);
    }

    #[test]
    fn compile_source_failures_render_source_context_and_summary() {
        let source = SourceFile::new(
            Utf8PathBuf::from("compile-source.fs"),
            "answer = unsupported".to_owned(),
        );
        let failure = CompileSourceFailed {
            message: "native lowering failed at compile-source.fs:1:10".to_owned(),
            title: "native lowering failed".to_owned(),
            location: "compile-source.fs:1:10".to_owned(),
            line_number: 1,
            source_line: "answer = unsupported".to_owned(),
            pointer_column: 10,
            pointer_width: 11,
            pointer_label: "unsupported here".to_owned(),
            before_lines: Vec::new(),
            after_lines: Vec::new(),
            src: source.named_source(),
            label: "the bootstrap compiler cannot lower this source construct yet".to_owned(),
            span: SourceSpan::from(fscript_source::Span::new(9, 20)),
        };
        let compile_failed = CompileFailed::Source(Box::new(failure));
        let error = DriverError::Compile(Box::new(compile_failed));

        let summary = error.diagnostic_summary();
        assert_eq!(summary.kind, "compile");
        assert_eq!(summary.title, "native lowering failed");
        assert_eq!(summary.line, Some(1));
        assert_eq!(summary.column, Some(10));
        assert_eq!(
            summary.label,
            Some("the bootstrap compiler cannot lower this source construct yet".to_owned())
        );

        let pretty = error.render_pretty();
        assert!(pretty.contains("native lowering failed"));
        assert!(pretty.contains("unsupported here"));

        let labels = match &error {
            DriverError::Compile(inner) => inner.labels(),
            other => panic!("expected compile error, found {other:?}"),
        }
        .expect("compile source errors should expose labels")
        .collect::<Vec<_>>();
        assert_eq!(labels.len(), 1);
        assert!(matches!(&error, DriverError::Compile(inner) if inner.source_code().is_some()));
    }

    #[test]
    fn runtime_failed_helper_constructors_produce_expected_messages() {
        let cases = [
            RuntimeFailed::from_message("plain runtime failure"),
            RuntimeFailed::unknown_identifier("missing".to_owned()),
            RuntimeFailed::duplicate_binding("value".to_owned()),
            RuntimeFailed::unknown_std_module("std:nope"),
            RuntimeFailed::unsupported_import("./other.fs"),
            RuntimeFailed::unknown_export("./other.fs", "greet"),
            RuntimeFailed::missing_property("name"),
            RuntimeFailed::unsupported("pipelines"),
            RuntimeFailed::unsupported_call(&Value::Number(42.0)),
        ];

        let messages = cases
            .into_iter()
            .map(DriverError::from)
            .map(|error| error.diagnostic_summary().message)
            .collect::<Vec<_>>();

        assert!(
            messages
                .iter()
                .any(|message| message == "plain runtime failure")
        );
        assert!(
            messages
                .iter()
                .any(|message| message == "unknown identifier `missing`")
        );
        assert!(
            messages
                .iter()
                .any(|message| message == "binding `value` is already defined in this scope")
        );
        assert!(
            messages
                .iter()
                .any(|message| message == "unknown standard library module `std:nope`")
        );
        assert!(
            messages
                .iter()
                .any(|message| message
                    == "runtime imports from `./other.fs` are not implemented yet")
        );
        assert!(
            messages
                .iter()
                .any(|message| message == "module `./other.fs` does not export `greet`")
        );
        assert!(
            messages
                .iter()
                .any(|message| message == "record does not contain a `name` field")
        );
        assert!(
            messages
                .iter()
                .any(|message| message == "runtime support for pipelines is not implemented yet")
        );
        assert!(
            messages
                .iter()
                .any(|message| message == "cannot call value `42`")
        );
    }

    #[test]
    fn lex_and_parse_failure_helpers_cover_all_diagnostic_kinds() {
        let source = SourceFile::new(
            Utf8PathBuf::from("diagnostics.fs"),
            "@\nmessage 'hello'\n".to_owned(),
        );

        let lex_error = LexFailed::from_source(
            &source,
            vec![
                LexDiagnostic {
                    kind: LexDiagnosticKind::InvalidToken('@'),
                    span: Span::new(0, 1),
                },
                LexDiagnostic {
                    kind: LexDiagnosticKind::UnterminatedString,
                    span: Span::new(2, 3),
                },
                LexDiagnostic {
                    kind: LexDiagnosticKind::InvalidEscape('x'),
                    span: Span::new(4, 6),
                },
                LexDiagnostic {
                    kind: LexDiagnosticKind::UnterminatedBlockComment,
                    span: Span::new(7, 9),
                },
            ],
        );
        assert!(render_source_error(lex_error.render_context()).contains("problem starts here"));
        assert_eq!(
            lex_error
                .labels()
                .expect("lex errors expose labels")
                .collect::<Vec<_>>()
                .len(),
            1
        );
        assert!(lex_error.source_code().is_some());

        let parse_error = ParseFailed::from_source(
            &source,
            vec![
                ParseDiagnostic {
                    kind: ParseDiagnosticKind::Expected("identifier"),
                    span: Span::new(0, 1),
                },
                ParseDiagnostic {
                    kind: ParseDiagnosticKind::UnexpectedToken(TokenKind::Assign),
                    span: Span::new(2, 3),
                },
                ParseDiagnostic {
                    kind: ParseDiagnosticKind::InvalidModuleItem,
                    span: Span::new(4, 5),
                },
                ParseDiagnostic {
                    kind: ParseDiagnosticKind::InvalidPattern,
                    span: Span::new(6, 7),
                },
                ParseDiagnostic {
                    kind: ParseDiagnosticKind::InvalidType,
                    span: Span::new(8, 9),
                },
            ],
        );
        assert!(render_source_error(parse_error.render_context()).contains("unexpected here"));
        assert_eq!(
            parse_error
                .labels()
                .expect("parse errors expose labels")
                .collect::<Vec<_>>()
                .len(),
            1
        );
        assert!(parse_error.source_code().is_some());
    }

    #[test]
    fn compile_failed_helpers_cover_tool_and_program_fallback_paths() {
        let tool = CompileFailed::from_message("tool-only failure");
        assert_eq!(tool.to_string(), "tool-only failure");
        assert!(tool.labels().is_none());
        assert!(tool.source_code().is_none());
        assert_eq!(tool.render_pretty(), "  × tool-only failure");

        let program = LoadedProgram {
            entry_path: Utf8PathBuf::from("entry.fs"),
            entry_token_count: 0,
            modules: BTreeMap::new(),
        };
        let compile = CompileFailed::from_program(
            &program,
            CompileError::ProgramImage {
                details: "broken image".to_owned(),
            },
        );
        assert!(matches!(compile, CompileFailed::Tool { .. }));
        assert!(
            compile
                .to_string()
                .contains("failed to encode the embedded program image")
        );
    }

    #[test]
    fn shared_runtime_entrypoints_cover_run_modules_and_runtime_value_conversion() {
        let modules = BTreeMap::from([(
            "<entry>".to_owned(),
            fscript_ir::Module {
                items: vec![fscript_ir::ModuleItem::Binding(fscript_ir::BindingDecl {
                    pattern: fscript_ir::Pattern::Identifier {
                        name: "value".to_owned(),
                        span: span(),
                    },
                    value: fscript_ir::Expr::StringLiteral {
                        value: "hello".to_owned(),
                        span: span(),
                    },
                    is_exported: false,
                    span: span(),
                })],
                exports: vec![],
            },
        )]);
        let last_value =
            run_modules(&modules, "<entry>").expect("shared runtime execution should work");
        assert_eq!(last_value, Some(Value::String("hello".to_owned())));

        let deferred = Value::from_runtime_value(super::shared_runtime::Value::Deferred(
            super::shared_runtime::DeferredValue::new(super::shared_runtime::DeferredBody::Expr {
                expr: Box::new(fscript_ir::Expr::Undefined { span: span() }),
                environment: super::shared_runtime::Environment::new(),
            }),
        ));
        assert!(matches!(deferred, Value::Deferred(_)));

        let function = Value::from_runtime_value(super::shared_runtime::Value::Function(
            super::shared_runtime::FunctionValue {
                parameters: vec![fscript_ir::Parameter {
                    pattern: fscript_ir::Pattern::Identifier {
                        name: "value".to_owned(),
                        span: span(),
                    },
                    span: span(),
                }],
                body: Box::new(fscript_ir::Expr::Undefined { span: span() }),
                environment: super::shared_runtime::Environment::new(),
                applied_args: Vec::new(),
                is_generator: false,
            },
        ));
        assert!(matches!(function, Value::Function(_)));

        let native = Value::from_runtime_value(super::shared_runtime::Value::NativeFunction(
            super::shared_runtime::NativeFunctionValue::new(
                super::shared_runtime::NativeFunction::TaskForce,
            ),
        ));
        assert!(matches!(native, Value::NativeFunction(_)));
    }

    #[test]
    fn source_helpers_cover_in_memory_paths_and_formatting() {
        let summary = check_source("answer = 42").expect("in-memory source should check");
        assert_eq!(summary.path, Utf8PathBuf::from("sandbox.fs"));
        assert!(summary.token_count >= 3);

        let source = SourceFile::new(
            Utf8PathBuf::from("example.fs"),
            "first = 1\nsecond = 'Ada' // trailing comment\nthird = 3\n".to_owned(),
        );
        assert_eq!(
            collect_before_lines(&source, 2, 2),
            vec![(1, "first = 1".to_owned())]
        );
        assert_eq!(
            collect_after_lines(&source, 2, 3),
            vec![(3, "third = 3".to_owned())]
        );

        let highlighted = highlight_source_line("name = 'Ada' // trailing comment");
        assert!(highlighted.contains("\u{1b}[32m'Ada'\u{1b}[0m"));
        assert!(highlighted.contains("\u{1b}[90m// trailing comment\u{1b}[0m"));

        let rendered = render_source_error(super::SourceRenderContext {
            title: "invalid type",
            location: "example.fs:2:5",
            line_number: 2,
            before_lines: &[(1, "answer = 1".to_owned())],
            source_line: "value = 'Ada'",
            pointer_column: 9,
            pointer_width: 5,
            pointer_label: "problem starts here",
            after_lines: &[(3, "other = value".to_owned())],
        });
        assert!(rendered.contains("invalid type"));
        assert!(rendered.contains("problem starts here"));
        assert!(rendered.contains("other = value"));
    }

    #[test]
    fn import_resolution_and_in_memory_loading_cover_error_paths() {
        let error = resolve_import_path(Utf8Path::new("."), "pkg:thing")
            .expect_err("non-relative imports should fail");
        assert!(matches!(error, DriverError::Import(_)));
        assert!(
            error
                .to_string()
                .contains("only relative user-module imports are supported")
        );

        let module = fscript_ir::Module {
            items: vec![fscript_ir::ModuleItem::Import(fscript_ir::ImportDecl {
                clause: fscript_ir::ImportClause::Default("value".to_owned()),
                source: "./other.fs".to_owned(),
                source_span: span(),
                span: span(),
            })],
            exports: vec![],
        };
        let error = reject_non_std_imports(&module).expect_err("user imports should fail");
        assert!(matches!(error, DriverError::Import(_)));

        let error =
            load_module_from_source(Utf8PathBuf::from("sandbox.txt"), "answer = 42".to_owned())
                .expect_err("non-.fs in-memory sources should fail");
        assert!(matches!(error, DriverError::UnsupportedExtension(_)));
    }

    #[test]
    fn bootstrap_runtime_imports_and_bindings_cover_failure_paths() {
        let mut environment = Environment::new();
        let unsupported = ImportDecl {
            clause: ImportClause::Default(identifier("Array")),
            source: "./user.fs".to_owned(),
            source_span: span(),
            span: span(),
        };
        let error = load_import(&unsupported, &mut environment)
            .expect_err("user imports should stay unsupported in the bootstrap runtime");
        assert!(
            error.diagnostics[0]
                .message
                .contains("runtime imports from `./user.fs`")
        );

        let mut environment = Environment::new();
        let unknown_std = ImportDecl {
            clause: ImportClause::Default(identifier("Unknown")),
            source: "std:unknown".to_owned(),
            source_span: span(),
            span: span(),
        };
        let error = load_import(&unknown_std, &mut environment)
            .expect_err("unknown std modules should fail");
        assert!(
            error.diagnostics[0]
                .message
                .contains("unknown standard library module `std:unknown`")
        );

        let mut environment = Environment::new();
        let missing_named = ImportDecl {
            clause: ImportClause::Named(vec![identifier("missing")]),
            source: "std:array".to_owned(),
            source_span: span(),
            span: span(),
        };
        let error = load_import(&missing_named, &mut environment)
            .expect_err("missing named exports should fail");
        assert!(
            error.diagnostics[0]
                .message
                .contains("module `std:array` does not export `missing`")
        );

        define_binding(&mut environment, "value", Value::Number(1.0))
            .expect("the first binding should succeed");
        let duplicate = define_binding(&mut environment, "value", Value::Number(2.0))
            .expect_err("duplicate bindings should fail");
        assert!(
            duplicate.diagnostics[0]
                .message
                .contains("binding `value` is already defined in this scope")
        );
    }

    #[test]
    fn bootstrap_runtime_patterns_and_calls_cover_mismatch_paths() {
        let duplicate_pattern = Pattern::Record {
            fields: vec![
                RecordPatternField {
                    name: identifier("name"),
                    pattern: None,
                    span: span(),
                },
                RecordPatternField {
                    name: identifier("name"),
                    pattern: None,
                    span: span(),
                },
            ],
            span: span(),
        };
        let duplicate_match = match_pattern(
            &duplicate_pattern,
            &Value::Record(BTreeMap::from([(
                "name".to_owned(),
                Value::String("Ada".to_owned()),
            )])),
        )
        .expect_err("duplicate pattern bindings should fail");
        assert!(
            duplicate_match.diagnostics[0]
                .message
                .contains("binding `name` is already defined in this scope")
        );

        let mismatch_pattern = Pattern::Array {
            items: vec![identifier_pattern("first"), identifier_pattern("second")],
            span: span(),
        };
        assert_eq!(
            match_pattern(&mismatch_pattern, &Value::Array(vec![Value::Number(1.0)]))
                .expect("length mismatches should not error"),
            None
        );

        let pattern_call_error = super::call_value(
            function_with_parameter_pattern(Pattern::Literal(LiteralPattern::String {
                value: "Ada".to_owned(),
                span: span(),
            })),
            vec![Value::String("Grace".to_owned())],
            None,
        )
        .expect_err("non-matching parameter patterns should fail");
        let pattern_call_error = runtime_control_to_failed(pattern_call_error);
        assert!(
            pattern_call_error.diagnostics[0]
                .message
                .contains("does not match binding pattern")
        );
    }

    #[test]
    fn bootstrap_runtime_unary_index_and_native_errors_are_reported() {
        let unary = evaluate_unary_expr(UnaryOperator::Not, Value::Number(1.0))
            .expect_err("boolean not should reject numbers");
        assert!(
            unary.diagnostics[0]
                .message
                .contains("cannot apply `!` to value `1`")
        );

        let deferred = evaluate_unary_expr(
            UnaryOperator::Defer,
            Value::Deferred(DeferredValue {
                expr: Box::new(number(1.0)),
                environment: Environment::new(),
                resolved: Rc::new(RefCell::new(Some(EvalOutcome::Value(Value::Number(1.0))))),
            }),
        )
        .expect_err("bootstrap runtime should reject direct defer unary evaluation");
        assert!(
            deferred.diagnostics[0]
                .message
                .contains("runtime support for `defer` expressions")
        );

        let fractional =
            evaluate_index_expr(Value::Array(vec![Value::Number(1.0)]), Value::Number(1.5))
                .expect_err("fractional indexes should fail");
        assert!(
            fractional.diagnostics[0]
                .message
                .contains("array indexes must be non-negative whole numbers")
        );

        let oob = evaluate_index_expr(
            Value::Sequence(vec![Value::Number(1.0)]),
            Value::Number(2.0),
        )
        .expect_err("out-of-bounds indexes should fail");
        assert!(
            oob.diagnostics[0]
                .message
                .contains("index `2` is out of bounds")
        );

        let wrong_object = evaluate_index_expr(Value::String("Ada".to_owned()), Value::Number(0.0))
            .expect_err("non-arrays should fail");
        assert!(
            wrong_object.diagnostics[0]
                .message
                .contains("cannot index into value `Ada`")
        );

        let shared_runtime_native = call_native_function(
            NativeFunctionValue::new(NativeFunction::TaskForce),
            vec![Value::Number(1.0)],
            None,
        )
        .expect_err("shared-runtime-only natives should fail in the bootstrap runtime");
        let shared_runtime_native = runtime_control_to_failed(shared_runtime_native);
        assert!(
            shared_runtime_native.diagnostics[0]
                .message
                .contains("only available through the shared runtime path")
        );

        let too_many_native_args = call_native_function(
            NativeFunctionValue::new(NativeFunction::ResultOk),
            vec![Value::Number(1.0), Value::Number(2.0)],
            None,
        )
        .expect_err("extra native arguments should fail");
        let too_many_native_args = runtime_control_to_failed(too_many_native_args);
        assert!(
            too_many_native_args.diagnostics[0]
                .message
                .contains("Result.ok expected 1 arguments but received 2")
        );
    }

    #[test]
    fn snapshots_parse_error_output() {
        let path = write_temp_file("parse-snapshot", "message 'hello'");
        let error = check_file(&path).expect_err("invalid syntax should fail");

        assert_snapshot!(
            "parse_error_output",
            normalize_snapshot(&error.render_pretty())
        );
    }

    #[test]
    fn snapshots_compile_error_output() {
        let path = write_temp_file("compile-snapshot", "answer = 42");
        let output_directory = write_temp_project("compile-snapshot-output-directory", &[]);
        let error = compile_file(&path, &output_directory).expect_err("tool failure should fail");
        let rendered = normalize_snapshot(&error.render_pretty());

        assert!(
            rendered.starts_with(
                "  × a native build tool failed while building `<tmp>/fscript-project-driver-compile-snapshot-output-directory`"
            ),
            "expected a stable compile tool failure prefix, got: {rendered}"
        );
    }
}
