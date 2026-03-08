//! High-level intermediate representation for the current semantic frontend.

use fscript_source::Span;

/// A lowered module with resolved names.
#[derive(Clone, Debug, PartialEq)]
pub struct Module {
    pub items: Vec<ModuleItem>,
}

/// A lowered top-level item.
#[derive(Clone, Debug, PartialEq)]
pub enum ModuleItem {
    Import(ImportDecl),
    Type(TypeDecl),
    Binding(BindingDecl),
}

/// A resolved import declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportDecl {
    pub clause: ImportClause,
    pub source: String,
    pub source_span: Span,
    pub span: Span,
}

/// Import bindings introduced into module scope.
#[derive(Clone, Debug, PartialEq)]
pub enum ImportClause {
    Default(BindingName),
    Named(Vec<BindingName>),
}

/// A resolved type declaration.
#[derive(Clone, Debug, PartialEq)]
pub struct TypeDecl {
    pub name: TypeName,
    pub type_params: Vec<TypeParam>,
    pub value: TypeExpr,
    pub is_exported: bool,
    pub span: Span,
}

/// A lowered immutable binding.
#[derive(Clone, Debug, PartialEq)]
pub struct BindingDecl {
    pub pattern: Pattern,
    pub value: Expr,
    pub is_exported: bool,
    pub span: Span,
}

/// A lowered function parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    pub pattern: Pattern,
    pub type_annotation: Option<TypeExpr>,
    pub span: Span,
}

/// A lowered block item.
#[derive(Clone, Debug, PartialEq)]
pub enum BlockItem {
    Binding(BindingDecl),
    Expr(Expr),
}

/// A lowered match arm.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

/// A lowered record literal field.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordField {
    pub name: String,
    pub span: Span,
    pub value: Expr,
}

/// A resolved binding declaration name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BindingName {
    pub id: BindingId,
    pub name: String,
    pub span: Span,
}

/// A resolved type declaration name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeName {
    pub id: TypeId,
    pub name: String,
    pub span: Span,
}

/// A resolved type parameter name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeParam {
    pub id: TypeParamId,
    pub name: String,
    pub span: Span,
}

/// A resolved identifier reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameRef {
    pub id: BindingId,
    pub name: String,
    pub span: Span,
}

/// Pattern forms after lowering.
#[derive(Clone, Debug, PartialEq)]
pub enum Pattern {
    Identifier(BindingName),
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
            Self::Identifier(binding) => binding.span,
            Self::Record { span, .. } | Self::Array { span, .. } => *span,
            Self::Literal(literal) => literal.span(),
        }
    }
}

/// A lowered record destructuring field.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordPatternField {
    pub name: String,
    pub binding: Option<BindingName>,
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

/// Lowered type syntax.
#[derive(Clone, Debug, PartialEq)]
pub enum TypeExpr {
    Reference {
        reference: TypeReference,
        span: Span,
    },
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
    Apply {
        callee: TypeReference,
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
}

impl TypeExpr {
    /// Returns the source span for the type expression.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Reference { span, .. }
            | Self::Record { span, .. }
            | Self::Function { span, .. }
            | Self::Apply { span, .. }
            | Self::Array { span, .. }
            | Self::Union { span, .. }
            | Self::Intersection { span, .. } => *span,
            Self::Literal(literal) => literal.span(),
        }
    }
}

/// A resolved type name reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeReference {
    BuiltinPrimitive(BuiltinPrimitive),
    BuiltinGeneric(BuiltinGeneric),
    Alias(TypeName),
    TypeParam(TypeParam),
}

/// Built-in primitive type names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinPrimitive {
    Number,
    String,
    Boolean,
    Null,
    Undefined,
    Never,
    Unknown,
}

/// Built-in generic type constructors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuiltinGeneric {
    Sequence,
    Result,
}

/// A lowered record type field.
#[derive(Clone, Debug, PartialEq)]
pub struct RecordTypeField {
    pub name: String,
    pub span: Span,
    pub value: TypeExpr,
}

/// A lowered function type parameter.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionTypeParam {
    pub name: String,
    pub span: Span,
    pub value: TypeExpr,
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

/// Binary operators ordered by precedence.
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

/// Lowered expression forms.
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
    Identifier(NameRef),
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
            Self::Identifier(identifier) => identifier.span,
        }
    }
}

/// A resolved binding identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BindingId(pub u32);

/// A resolved type declaration identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeId(pub u32);

/// A resolved type parameter identity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeParamId(pub u32);

#[cfg(test)]
mod tests {
    use super::*;

    const fn span(start: usize, end: usize) -> Span {
        Span::new(start, end)
    }

    fn binding(name: &str, index: u32, span: Span) -> BindingName {
        BindingName {
            id: BindingId(index),
            name: name.to_owned(),
            span,
        }
    }

    fn type_name(name: &str, index: u32, span: Span) -> TypeName {
        TypeName {
            id: TypeId(index),
            name: name.to_owned(),
            span,
        }
    }

    fn type_param(name: &str, index: u32, span: Span) -> TypeParam {
        TypeParam {
            id: TypeParamId(index),
            name: name.to_owned(),
            span,
        }
    }

    #[test]
    fn pattern_span_covers_every_variant() {
        let patterns = [
            Pattern::Identifier(binding("value", 0, span(0, 1))),
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

        assert_eq!(patterns[0].span(), span(0, 1));
        assert_eq!(patterns[1].span(), span(1, 2));
        assert_eq!(patterns[2].span(), span(2, 3));
        assert_eq!(patterns[3].span(), span(3, 4));
    }

    #[test]
    fn literal_pattern_and_literal_type_spans_cover_every_variant() {
        let literal_patterns = [
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
        let literal_types = [
            LiteralType::String {
                value: "x".to_owned(),
                span: span(5, 6),
            },
            LiteralType::Number {
                value: 1.0,
                span: span(6, 7),
            },
            LiteralType::Boolean {
                value: true,
                span: span(7, 8),
            },
        ];

        assert_eq!(literal_patterns[0].span(), span(0, 1));
        assert_eq!(literal_patterns[1].span(), span(1, 2));
        assert_eq!(literal_patterns[2].span(), span(2, 3));
        assert_eq!(literal_patterns[3].span(), span(3, 4));
        assert_eq!(literal_patterns[4].span(), span(4, 5));
        assert_eq!(literal_types[0].span(), span(5, 6));
        assert_eq!(literal_types[1].span(), span(6, 7));
        assert_eq!(literal_types[2].span(), span(7, 8));
    }

    #[test]
    fn type_expr_span_covers_every_variant() {
        let literal = LiteralType::String {
            value: "x".to_owned(),
            span: span(1, 2),
        };
        let items = [
            TypeExpr::Reference {
                reference: TypeReference::Alias(type_name("Thing", 0, span(0, 1))),
                span: span(0, 1),
            },
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
            TypeExpr::Apply {
                callee: TypeReference::TypeParam(type_param("T", 1, span(4, 5))),
                args: Vec::new(),
                span: span(4, 5),
            },
            TypeExpr::Array {
                element: Box::new(TypeExpr::Literal(literal.clone())),
                span: span(5, 6),
            },
            TypeExpr::Union {
                members: Vec::new(),
                span: span(6, 7),
            },
            TypeExpr::Intersection {
                members: Vec::new(),
                span: span(7, 8),
            },
        ];

        assert_eq!(items[0].span(), span(0, 1));
        assert_eq!(items[1].span(), span(1, 2));
        assert_eq!(items[2].span(), span(2, 3));
        assert_eq!(items[3].span(), span(3, 4));
        assert_eq!(items[4].span(), span(4, 5));
        assert_eq!(items[5].span(), span(5, 6));
        assert_eq!(items[6].span(), span(6, 7));
        assert_eq!(items[7].span(), span(7, 8));
    }

    #[test]
    fn expr_span_covers_every_variant() {
        let id = NameRef {
            id: BindingId(9),
            name: "value".to_owned(),
            span: span(5, 6),
        };
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
                catch_pattern: Pattern::Identifier(binding("error", 1, span(12, 13))),
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
                callee: Box::new(Expr::Identifier(id.clone())),
                args: Vec::new(),
                span: span(17, 18),
            },
            Expr::Member {
                object: Box::new(Expr::Identifier(id.clone())),
                property: "field".to_owned(),
                span: span(18, 19),
            },
            Expr::Index {
                object: Box::new(Expr::Identifier(id)),
                index: Box::new(Expr::NumberLiteral {
                    value: 0.0,
                    span: span(19, 20),
                }),
                span: span(19, 20),
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
