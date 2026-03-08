//! Shared interpreter and codegen IR for the first executable slice.

use std::collections::BTreeMap;

use fscript_source::Span;
use serde::{Deserialize, Serialize};

/// A serialized executable image consumed by compiled launcher binaries.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompiledProgram {
    pub entry: String,
    pub modules: BTreeMap<String, Module>,
}

/// A lowered executable module.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Module {
    pub items: Vec<ModuleItem>,
    pub exports: Vec<String>,
}

/// Executable module items.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ModuleItem {
    Import(ImportDecl),
    Binding(BindingDecl),
}

/// A lowered import declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImportDecl {
    pub clause: ImportClause,
    pub source: String,
    pub source_span: Span,
    pub span: Span,
}

/// Import bindings introduced into module scope.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ImportClause {
    Default(String),
    Named(Vec<String>),
}

/// A lowered immutable binding declaration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BindingDecl {
    pub pattern: Pattern,
    pub value: Expr,
    pub is_exported: bool,
    pub span: Span,
}

/// A lowered block item.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BlockItem {
    Binding(BindingDecl),
    Expr(Expr),
}

/// A lowered function parameter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub pattern: Pattern,
    pub span: Span,
}

/// A lowered record field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordField {
    pub name: String,
    pub value: Expr,
    pub span: Span,
}

/// A lowered match arm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// Pattern forms used by the runtime layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pattern {
    Identifier {
        name: String,
        span: Span,
    },
    Record {
        fields: Vec<RecordPatternField>,
        span: Span,
    },
    Array {
        items: Vec<Pattern>,
        span: Span,
    },
    Literal(LiteralPattern),
}

impl Pattern {
    /// Returns the source span for the pattern.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Identifier { span, .. }
            | Self::Record { span, .. }
            | Self::Array { span, .. } => *span,
            Self::Literal(literal) => literal.span(),
        }
    }
}

/// A lowered record destructuring field.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RecordPatternField {
    pub name: String,
    pub binding: Option<String>,
    pub pattern: Option<Box<Pattern>>,
    pub span: Span,
}

/// Literal pattern forms.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum LiteralPattern {
    String { value: String, span: Span },
    Number { value: f64, span: Span },
    Boolean { value: bool, span: Span },
    Null { span: Span },
    Undefined { span: Span },
}

impl LiteralPattern {
    /// Returns the source span for the pattern.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::String { span, .. }
            | Self::Number { span, .. }
            | Self::Boolean { span, .. }
            | Self::Null { span }
            | Self::Undefined { span } => *span,
        }
    }
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UnaryOperator {
    Not,
    Negate,
    Positive,
    Defer,
}

/// Binary operators ordered by precedence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOperator {
    LogicalOr,
    LogicalAnd,
    NullishCoalesce,
    StrictEqual,
    StrictNotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Add,
    Subtract,
    Multiply,
    Divide,
    Modulo,
}

/// Executable expression forms.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    StringLiteral {
        value: String,
        span: Span,
    },
    NumberLiteral {
        value: f64,
        span: Span,
    },
    BooleanLiteral {
        value: bool,
        span: Span,
    },
    Null {
        span: Span,
    },
    Undefined {
        span: Span,
    },
    Identifier {
        name: String,
        span: Span,
    },
    Record {
        fields: Vec<RecordField>,
        span: Span,
    },
    Array {
        items: Vec<Expr>,
        span: Span,
    },
    Function {
        parameters: Vec<Parameter>,
        body: Box<Expr>,
        is_generator: bool,
        span: Span,
    },
    Block {
        items: Vec<BlockItem>,
        span: Span,
    },
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
        span: Span,
    },
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    Try {
        body: Box<Expr>,
        catch_pattern: Pattern,
        catch_body: Box<Expr>,
        span: Span,
    },
    Throw {
        value: Box<Expr>,
        span: Span,
    },
    Yield {
        value: Box<Expr>,
        span: Span,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Expr>,
        span: Span,
    },
    Binary {
        operator: BinaryOperator,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },
    Member {
        object: Box<Expr>,
        property: String,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
}

impl Expr {
    /// Returns the source span for the expression.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::StringLiteral { span, .. }
            | Self::NumberLiteral { span, .. }
            | Self::BooleanLiteral { span, .. }
            | Self::Null { span }
            | Self::Undefined { span }
            | Self::Identifier { span, .. }
            | Self::Record { span, .. }
            | Self::Array { span, .. }
            | Self::Function { span, .. }
            | Self::Block { span, .. }
            | Self::If { span, .. }
            | Self::Match { span, .. }
            | Self::Try { span, .. }
            | Self::Throw { span, .. }
            | Self::Yield { span, .. }
            | Self::Unary { span, .. }
            | Self::Binary { span, .. }
            | Self::Call { span, .. }
            | Self::Member { span, .. }
            | Self::Index { span, .. } => *span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn span(start: usize, end: usize) -> Span {
        Span::new(start, end)
    }

    #[test]
    fn pattern_and_literal_pattern_spans_cover_every_variant() {
        let patterns = [
            Pattern::Identifier {
                name: "value".to_owned(),
                span: span(0, 1),
            },
            Pattern::Record {
                fields: Vec::new(),
                span: span(1, 2),
            },
            Pattern::Array {
                items: Vec::new(),
                span: span(2, 3),
            },
            Pattern::Literal(LiteralPattern::Null { span: span(3, 4) }),
        ];
        let literals = [
            LiteralPattern::String {
                value: "x".to_owned(),
                span: span(4, 5),
            },
            LiteralPattern::Number {
                value: 1.0,
                span: span(5, 6),
            },
            LiteralPattern::Boolean {
                value: true,
                span: span(6, 7),
            },
            LiteralPattern::Null { span: span(7, 8) },
            LiteralPattern::Undefined { span: span(8, 9) },
        ];

        assert_eq!(patterns[0].span(), span(0, 1));
        assert_eq!(patterns[1].span(), span(1, 2));
        assert_eq!(patterns[2].span(), span(2, 3));
        assert_eq!(patterns[3].span(), span(3, 4));
        assert_eq!(literals[0].span(), span(4, 5));
        assert_eq!(literals[1].span(), span(5, 6));
        assert_eq!(literals[2].span(), span(6, 7));
        assert_eq!(literals[3].span(), span(7, 8));
        assert_eq!(literals[4].span(), span(8, 9));
    }

    #[test]
    fn expr_span_covers_every_variant() {
        let expressions = [
            Expr::StringLiteral {
                value: "x".to_owned(),
                span: span(0, 1),
            },
            Expr::NumberLiteral {
                value: 1.0,
                span: span(1, 2),
            },
            Expr::BooleanLiteral {
                value: true,
                span: span(2, 3),
            },
            Expr::Null { span: span(3, 4) },
            Expr::Undefined { span: span(4, 5) },
            Expr::Identifier {
                name: "value".to_owned(),
                span: span(5, 6),
            },
            Expr::Record {
                fields: Vec::new(),
                span: span(6, 7),
            },
            Expr::Array {
                items: Vec::new(),
                span: span(7, 8),
            },
            Expr::Function {
                parameters: Vec::new(),
                body: Box::new(Expr::Null { span: span(8, 9) }),
                is_generator: false,
                span: span(8, 9),
            },
            Expr::Block {
                items: Vec::new(),
                span: span(9, 10),
            },
            Expr::If {
                condition: Box::new(Expr::BooleanLiteral {
                    value: true,
                    span: span(10, 11),
                }),
                then_branch: Box::new(Expr::Null { span: span(10, 11) }),
                else_branch: Some(Box::new(Expr::Undefined { span: span(10, 11) })),
                span: span(10, 11),
            },
            Expr::Match {
                value: Box::new(Expr::Null { span: span(11, 12) }),
                arms: Vec::new(),
                span: span(11, 12),
            },
            Expr::Try {
                body: Box::new(Expr::Null { span: span(12, 13) }),
                catch_pattern: Pattern::Identifier {
                    name: "error".to_owned(),
                    span: span(12, 13),
                },
                catch_body: Box::new(Expr::Undefined { span: span(12, 13) }),
                span: span(12, 13),
            },
            Expr::Throw {
                value: Box::new(Expr::Null { span: span(13, 14) }),
                span: span(13, 14),
            },
            Expr::Yield {
                value: Box::new(Expr::Null { span: span(14, 15) }),
                span: span(14, 15),
            },
            Expr::Unary {
                operator: UnaryOperator::Not,
                operand: Box::new(Expr::Null { span: span(15, 16) }),
                span: span(15, 16),
            },
            Expr::Binary {
                operator: BinaryOperator::Add,
                left: Box::new(Expr::NumberLiteral {
                    value: 1.0,
                    span: span(16, 17),
                }),
                right: Box::new(Expr::NumberLiteral {
                    value: 2.0,
                    span: span(16, 17),
                }),
                span: span(16, 17),
            },
            Expr::Call {
                callee: Box::new(Expr::Identifier {
                    name: "fn".to_owned(),
                    span: span(17, 18),
                }),
                args: Vec::new(),
                span: span(17, 18),
            },
            Expr::Member {
                object: Box::new(Expr::Identifier {
                    name: "record".to_owned(),
                    span: span(18, 19),
                }),
                property: "field".to_owned(),
                span: span(18, 19),
            },
            Expr::Index {
                object: Box::new(Expr::Identifier {
                    name: "array".to_owned(),
                    span: span(19, 20),
                }),
                index: Box::new(Expr::NumberLiteral {
                    value: 0.0,
                    span: span(19, 20),
                }),
                span: span(19, 20),
            },
        ];

        for (index, expr) in expressions.iter().enumerate() {
            assert_eq!(expr.span(), span(index, index + 1));
        }
    }
}
