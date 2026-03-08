//! Syntax tree definitions for FScript.

use fscript_source::Span;

/// A parsed FScript module.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    /// Top-level items in source order.
    pub items: Vec<ModuleItem>,
}

/// A top-level module item.
#[derive(Clone, Debug, PartialEq)]
pub enum ModuleItem {
    Import(ImportDecl),
    ExportBinding(BindingDecl),
    ExportType(TypeDecl),
    Type(TypeDecl),
    Binding(BindingDecl),
}

/// An import declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportDecl {
    pub clause: ImportClause,
    pub source: String,
    pub source_span: Span,
    pub span: Span,
}

/// The import bindings introduced by an import declaration.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportClause {
    Default(Identifier),
    Named(Vec<Identifier>),
}

/// A type declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeDecl {
    pub name: Identifier,
    pub type_params: Vec<Identifier>,
    pub value: TypeExpr,
    pub span: Span,
}

/// An immutable binding declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct BindingDecl {
    pub pattern: Pattern,
    pub value: Expr,
    pub span: Span,
}

/// A function parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub pattern: Pattern,
    pub type_annotation: Option<TypeExpr>,
    pub span: Span,
}

/// A block expression item.
#[derive(Clone, Debug, PartialEq)]
pub enum BlockItem {
    Binding(BindingDecl),
    Expr(Expr),
}

/// A match arm.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// A record literal field.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordField {
    pub name: Identifier,
    pub value: Expr,
    pub span: Span,
}

/// An identifier with span information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

/// Pattern forms supported by the grammar.
#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    Identifier(Identifier),
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
            Self::Identifier(identifier) => identifier.span,
            Self::Record { span, .. } | Self::Array { span, .. } => *span,
            Self::Literal(literal) => literal.span(),
        }
    }
}

/// A record destructuring field.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordPatternField {
    pub name: Identifier,
    pub pattern: Option<Box<Pattern>>,
    pub span: Span,
}

/// Literal pattern forms.
#[derive(Clone, Debug, PartialEq)]
pub enum LiteralPattern {
    String { value: String, span: Span },
    Number { value: f64, span: Span },
    Boolean { value: bool, span: Span },
    Null { span: Span },
    Undefined { span: Span },
}

impl LiteralPattern {
    /// Returns the source span for the literal pattern.
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

/// FScript type syntax.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeExpr {
    Identifier(Identifier),
    Literal(LiteralType),
    Record {
        fields: Vec<RecordTypeField>,
        span: Span,
    },
    Function {
        params: Vec<FunctionTypeParam>,
        return_type: Box<TypeExpr>,
        span: Span,
    },
    Generic {
        name: Identifier,
        args: Vec<TypeExpr>,
        span: Span,
    },
    Array {
        element: Box<TypeExpr>,
        span: Span,
    },
    Union {
        members: Vec<TypeExpr>,
        span: Span,
    },
    Intersection {
        members: Vec<TypeExpr>,
        span: Span,
    },
    Grouped {
        inner: Box<TypeExpr>,
        span: Span,
    },
}

impl TypeExpr {
    /// Returns the source span for the type expression.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Identifier(identifier) => identifier.span,
            Self::Literal(literal) => literal.span(),
            Self::Record { span, .. }
            | Self::Function { span, .. }
            | Self::Generic { span, .. }
            | Self::Array { span, .. }
            | Self::Union { span, .. }
            | Self::Intersection { span, .. }
            | Self::Grouped { span, .. } => *span,
        }
    }
}

/// A record type field.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordTypeField {
    pub name: Identifier,
    pub value: TypeExpr,
    pub span: Span,
}

/// A function type parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionTypeParam {
    pub name: Identifier,
    pub value: TypeExpr,
    pub span: Span,
}

/// Literal type syntax.
#[derive(Clone, Debug, PartialEq)]
pub enum LiteralType {
    String { value: String, span: Span },
    Number { value: f64, span: Span },
    Boolean { value: bool, span: Span },
}

impl LiteralType {
    /// Returns the source span for the literal type.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::String { span, .. } | Self::Number { span, .. } | Self::Boolean { span, .. } => {
                *span
            }
        }
    }
}

/// Unary operators.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnaryOperator {
    Not,
    Negate,
    Positive,
    Defer,
}

/// Binary operators ordered by the grammar precedence tiers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Expression forms supported by the grammar.
#[derive(Clone, Debug, PartialEq)]
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
    Identifier(Identifier),
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
        return_type: Option<TypeExpr>,
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
    Pipe {
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
        property: Identifier,
        span: Span,
    },
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
        span: Span,
    },
    Grouped {
        inner: Box<Expr>,
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
            | Self::Pipe { span, .. }
            | Self::Call { span, .. }
            | Self::Member { span, .. }
            | Self::Index { span, .. }
            | Self::Grouped { span, .. } => *span,
            Self::Identifier(identifier) => identifier.span,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn span(start: usize, end: usize) -> Span {
        Span::new(start, end)
    }

    fn identifier(name: &str, span: Span) -> Identifier {
        Identifier {
            name: name.to_owned(),
            span,
        }
    }

    #[test]
    fn pattern_span_covers_every_variant() {
        let literal_span = span(7, 8);
        let patterns = [
            Pattern::Identifier(identifier("value", span(0, 1))),
            Pattern::Record {
                fields: Vec::new(),
                span: span(1, 2),
            },
            Pattern::Array {
                items: Vec::new(),
                span: span(2, 3),
            },
            Pattern::Literal(LiteralPattern::String {
                value: "x".to_owned(),
                span: literal_span,
            }),
        ];

        assert_eq!(patterns[0].span(), span(0, 1));
        assert_eq!(patterns[1].span(), span(1, 2));
        assert_eq!(patterns[2].span(), span(2, 3));
        assert_eq!(patterns[3].span(), literal_span);
    }

    #[test]
    fn literal_pattern_span_covers_every_variant() {
        let literals = [
            LiteralPattern::String {
                value: "x".to_owned(),
                span: span(0, 1),
            },
            LiteralPattern::Number {
                value: 1.0,
                span: span(1, 2),
            },
            LiteralPattern::Boolean {
                value: true,
                span: span(2, 3),
            },
            LiteralPattern::Null { span: span(3, 4) },
            LiteralPattern::Undefined { span: span(4, 5) },
        ];

        assert_eq!(literals[0].span(), span(0, 1));
        assert_eq!(literals[1].span(), span(1, 2));
        assert_eq!(literals[2].span(), span(2, 3));
        assert_eq!(literals[3].span(), span(3, 4));
        assert_eq!(literals[4].span(), span(4, 5));
    }

    #[test]
    fn type_expr_span_covers_every_variant() {
        let literal = LiteralType::String {
            value: "x".to_owned(),
            span: span(1, 2),
        };
        let variants = [
            TypeExpr::Identifier(identifier("Thing", span(0, 1))),
            TypeExpr::Literal(literal.clone()),
            TypeExpr::Record {
                fields: Vec::new(),
                span: span(2, 3),
            },
            TypeExpr::Function {
                params: Vec::new(),
                return_type: Box::new(TypeExpr::Literal(literal.clone())),
                span: span(3, 4),
            },
            TypeExpr::Generic {
                name: identifier("Result", span(4, 5)),
                args: Vec::new(),
                span: span(5, 6),
            },
            TypeExpr::Array {
                element: Box::new(TypeExpr::Literal(literal.clone())),
                span: span(6, 7),
            },
            TypeExpr::Union {
                members: Vec::new(),
                span: span(7, 8),
            },
            TypeExpr::Intersection {
                members: Vec::new(),
                span: span(8, 9),
            },
            TypeExpr::Grouped {
                inner: Box::new(TypeExpr::Literal(literal)),
                span: span(9, 10),
            },
        ];

        assert_eq!(variants[0].span(), span(0, 1));
        assert_eq!(variants[1].span(), span(1, 2));
        assert_eq!(variants[2].span(), span(2, 3));
        assert_eq!(variants[3].span(), span(3, 4));
        assert_eq!(variants[4].span(), span(5, 6));
        assert_eq!(variants[5].span(), span(6, 7));
        assert_eq!(variants[6].span(), span(7, 8));
        assert_eq!(variants[7].span(), span(8, 9));
        assert_eq!(variants[8].span(), span(9, 10));
    }

    #[test]
    fn literal_type_span_covers_every_variant() {
        let literals = [
            LiteralType::String {
                value: "x".to_owned(),
                span: span(0, 1),
            },
            LiteralType::Number {
                value: 1.0,
                span: span(1, 2),
            },
            LiteralType::Boolean {
                value: true,
                span: span(2, 3),
            },
        ];

        assert_eq!(literals[0].span(), span(0, 1));
        assert_eq!(literals[1].span(), span(1, 2));
        assert_eq!(literals[2].span(), span(2, 3));
    }

    #[test]
    fn expr_span_covers_every_variant() {
        let id = identifier("value", span(5, 6));
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
            Expr::Identifier(id.clone()),
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
                return_type: None,
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
                catch_pattern: Pattern::Identifier(identifier("error", span(12, 13))),
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
            Expr::Pipe {
                left: Box::new(Expr::Null { span: span(17, 18) }),
                right: Box::new(Expr::Identifier(id.clone())),
                span: span(17, 18),
            },
            Expr::Call {
                callee: Box::new(Expr::Identifier(id.clone())),
                args: Vec::new(),
                span: span(18, 19),
            },
            Expr::Member {
                object: Box::new(Expr::Identifier(id.clone())),
                property: identifier("field", span(19, 20)),
                span: span(19, 20),
            },
            Expr::Index {
                object: Box::new(Expr::Identifier(id.clone())),
                index: Box::new(Expr::NumberLiteral {
                    value: 0.0,
                    span: span(20, 21),
                }),
                span: span(20, 21),
            },
            Expr::Grouped {
                inner: Box::new(Expr::Identifier(id)),
                span: span(21, 22),
            },
        ];

        for (index, expr) in expressions.iter().enumerate() {
            let expected = if index == 5 {
                span(5, 6)
            } else {
                span(index, index + 1)
            };
            assert_eq!(expr.span(), expected);
        }
    }
}
