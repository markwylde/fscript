//! Recursive-descent parser for FScript.

use fscript_ast::{
    BinaryOperator, BindingDecl, BlockItem, Expr, FunctionTypeParam, Identifier, ImportClause,
    ImportDecl, LiteralPattern, LiteralType, MatchArm, Module, ModuleItem, Parameter, Pattern,
    RecordField, RecordPatternField, RecordTypeField, TypeDecl, TypeExpr, UnaryOperator,
};
use fscript_lexer::{Token, TokenKind};
use fscript_source::{SourceFile, Span};

/// Result of parsing a module.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedModule {
    /// Parsed syntax tree.
    pub module: Module,
    /// Collected parse diagnostics.
    pub diagnostics: Vec<ParseDiagnostic>,
}

/// A parser diagnostic with source span information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseDiagnostic {
    /// Diagnostic kind.
    pub kind: ParseDiagnosticKind,
    /// Span for the diagnostic.
    pub span: Span,
}

/// Parser failure kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseDiagnosticKind {
    Expected(&'static str),
    UnexpectedToken(TokenKind),
    InvalidModuleItem,
    InvalidPattern,
    InvalidType,
}

/// Parses a module from lexed tokens.
#[must_use]
pub fn parse_module(source: &SourceFile, tokens: &[Token]) -> ParsedModule {
    Parser::new(source, tokens).parse()
}

struct Parser<'a> {
    source: &'a SourceFile,
    tokens: Vec<Token>,
    index: usize,
    diagnostics: Vec<ParseDiagnostic>,
}

impl<'a> Parser<'a> {
    fn new(source: &'a SourceFile, tokens: &[Token]) -> Self {
        Self {
            source,
            tokens: tokens
                .iter()
                .copied()
                .filter(|token| !is_trivia(token.kind))
                .collect(),
            index: 0,
            diagnostics: Vec::new(),
        }
    }

    fn parse(mut self) -> ParsedModule {
        let mut items = Vec::new();

        while !self.is_at_end() {
            match self.parse_module_item() {
                Some(item) => items.push(item),
                None => self.recover_module_item(),
            }
        }

        ParsedModule {
            module: Module { items },
            diagnostics: self.diagnostics,
        }
    }

    fn parse_module_item(&mut self) -> Option<ModuleItem> {
        match self.current_kind()? {
            TokenKind::Import => self.parse_import_decl().map(ModuleItem::Import),
            TokenKind::Export => self.parse_export_decl(),
            TokenKind::Type => self.parse_type_decl().map(ModuleItem::Type),
            _ => self.parse_binding_decl().map(ModuleItem::Binding),
        }
    }

    fn parse_import_decl(&mut self) -> Option<ImportDecl> {
        let import_token = self.expect(TokenKind::Import, "import")?;
        let clause = self.parse_import_clause()?;
        self.expect(TokenKind::From, "`from` in import declaration")?;
        let source_token = self.expect(TokenKind::StringLiteral, "import source string")?;
        let source = decode_string(source_token.span.slice(self.source.contents()));

        Some(ImportDecl {
            clause,
            source,
            source_span: source_token.span,
            span: import_token.span.cover(source_token.span),
        })
    }

    fn parse_import_clause(&mut self) -> Option<ImportClause> {
        if self.current_kind().is_some_and(is_identifier_like) {
            return self.parse_identifier().map(ImportClause::Default);
        }

        self.expect(TokenKind::LeftBrace, "`{` to start a named import list")?;
        let mut names = Vec::new();

        while self.current_kind() != Some(TokenKind::RightBrace) {
            names.push(self.parse_identifier()?);

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightBrace) {
                    break;
                }
            } else {
                break;
            }
        }

        self.expect(TokenKind::RightBrace, "`}` to close a named import list")?;
        Some(ImportClause::Named(names))
    }

    fn parse_export_decl(&mut self) -> Option<ModuleItem> {
        self.expect(TokenKind::Export, "`export`")?;

        if self.current_kind() == Some(TokenKind::Type) {
            return self.parse_type_decl().map(ModuleItem::ExportType);
        }

        self.parse_binding_decl().map(ModuleItem::ExportBinding)
    }

    fn parse_type_decl(&mut self) -> Option<TypeDecl> {
        let type_token = self.expect(TokenKind::Type, "`type`")?;
        let name = self.parse_identifier()?;
        let type_params = self.parse_type_params()?;
        self.expect(TokenKind::Assign, "`=` after a type declaration name")?;
        let value = self.parse_type_expr()?;

        Some(TypeDecl {
            name,
            type_params,
            span: type_token.span.cover(value.span()),
            value,
        })
    }

    fn parse_type_params(&mut self) -> Option<Vec<Identifier>> {
        if self.current_kind() != Some(TokenKind::Less) {
            return Some(Vec::new());
        }

        self.bump();
        let mut params = Vec::new();

        while self.current_kind() != Some(TokenKind::Greater) {
            params.push(self.parse_identifier()?);

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::Greater) {
                    break;
                }
            } else {
                break;
            }
        }

        self.expect(TokenKind::Greater, "`>` to close type parameters")?;
        Some(params)
    }

    fn parse_binding_decl(&mut self) -> Option<BindingDecl> {
        let pattern = self.parse_pattern()?;
        self.expect(TokenKind::Assign, "`=` after a binding pattern")?;
        let value = self.parse_expr()?;
        self.report_adjacent_expression_token(value.span());

        Some(BindingDecl {
            span: pattern.span().cover(value.span()),
            pattern,
            value,
        })
    }

    fn parse_pattern(&mut self) -> Option<Pattern> {
        match self.current_kind() {
            Some(TokenKind::Identifier) => self.parse_identifier().map(Pattern::Identifier),
            Some(TokenKind::LeftBrace) => self.parse_record_pattern(),
            Some(TokenKind::LeftBracket) => self.parse_array_pattern(),
            Some(TokenKind::StringLiteral)
            | Some(TokenKind::NumberLiteral)
            | Some(TokenKind::True)
            | Some(TokenKind::False)
            | Some(TokenKind::Null)
            | Some(TokenKind::Undefined) => self.parse_literal_pattern().map(Pattern::Literal),
            Some(_) => {
                self.push_diagnostic(ParseDiagnosticKind::InvalidPattern, self.current_span());
                None
            }
            None => {
                self.push_diagnostic(ParseDiagnosticKind::Expected("pattern"), self.eof_span());
                None
            }
        }
    }

    fn parse_record_pattern(&mut self) -> Option<Pattern> {
        let start = self.expect(TokenKind::LeftBrace, "`{` to start a record pattern")?;
        let mut fields = Vec::new();

        while self.current_kind() != Some(TokenKind::RightBrace) {
            let name = self.parse_identifier()?;
            let field = if self.current_kind() == Some(TokenKind::Colon) {
                self.bump();
                let nested_pattern = self.parse_pattern()?;
                let span = name.span.cover(nested_pattern.span());
                RecordPatternField {
                    name,
                    pattern: Some(Box::new(nested_pattern)),
                    span,
                }
            } else {
                RecordPatternField {
                    span: name.span,
                    name,
                    pattern: None,
                }
            };

            fields.push(field);

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightBrace) {
                    break;
                }
            } else {
                break;
            }
        }

        let end = self.expect(TokenKind::RightBrace, "`}` to close a record pattern")?;

        Some(Pattern::Record {
            fields,
            span: start.span.cover(end.span),
        })
    }

    fn parse_array_pattern(&mut self) -> Option<Pattern> {
        let start = self.expect(TokenKind::LeftBracket, "`[` to start an array pattern")?;
        let mut items = Vec::new();

        while self.current_kind() != Some(TokenKind::RightBracket) {
            items.push(self.parse_pattern()?);

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightBracket) {
                    break;
                }
            } else {
                break;
            }
        }

        let end = self.expect(TokenKind::RightBracket, "`]` to close an array pattern")?;

        Some(Pattern::Array {
            items,
            span: start.span.cover(end.span),
        })
    }

    fn parse_literal_pattern(&mut self) -> Option<LiteralPattern> {
        let token = self.bump()?;

        Some(match token.kind {
            TokenKind::StringLiteral => LiteralPattern::String {
                value: decode_string(token.span.slice(self.source.contents())),
                span: token.span,
            },
            TokenKind::NumberLiteral => LiteralPattern::Number {
                value: token
                    .span
                    .slice(self.source.contents())
                    .parse()
                    .unwrap_or_default(),
                span: token.span,
            },
            TokenKind::True => LiteralPattern::Boolean {
                value: true,
                span: token.span,
            },
            TokenKind::False => LiteralPattern::Boolean {
                value: false,
                span: token.span,
            },
            TokenKind::Null => LiteralPattern::Null { span: token.span },
            TokenKind::Undefined => LiteralPattern::Undefined { span: token.span },
            other => {
                self.push_diagnostic(ParseDiagnosticKind::UnexpectedToken(other), token.span);
                return None;
            }
        })
    }

    fn parse_type_expr(&mut self) -> Option<TypeExpr> {
        if self.current_kind() == Some(TokenKind::Union) {
            self.bump();
        }

        self.parse_union_type()
    }

    fn parse_union_type(&mut self) -> Option<TypeExpr> {
        let mut members = vec![self.parse_intersection_type()?];

        while self.current_kind() == Some(TokenKind::Union) {
            self.bump();
            members.push(self.parse_intersection_type()?);
        }

        if members.len() == 1 {
            members.pop()
        } else {
            let span = members[0]
                .span()
                .cover(members[members.len().saturating_sub(1)].span());
            Some(TypeExpr::Union { members, span })
        }
    }

    fn parse_intersection_type(&mut self) -> Option<TypeExpr> {
        let mut members = vec![self.parse_postfix_type()?];

        while self.current_kind() == Some(TokenKind::Intersection) {
            self.bump();
            members.push(self.parse_postfix_type()?);
        }

        if members.len() == 1 {
            members.pop()
        } else {
            let span = members[0]
                .span()
                .cover(members[members.len().saturating_sub(1)].span());
            Some(TypeExpr::Intersection { members, span })
        }
    }

    fn parse_postfix_type(&mut self) -> Option<TypeExpr> {
        let mut ty = self.parse_primary_type()?;

        while self.current_kind() == Some(TokenKind::LeftBracket)
            && self.nth_kind(1) == Some(TokenKind::RightBracket)
        {
            let start = ty.span();
            self.bump();
            let end = self.expect(TokenKind::RightBracket, "`]` in an array type")?;
            ty = TypeExpr::Array {
                element: Box::new(ty),
                span: start.cover(end.span),
            };
        }

        Some(ty)
    }

    fn parse_primary_type(&mut self) -> Option<TypeExpr> {
        if let Some(function_type) = self.parse_tentatively(|parser| parser.parse_function_type()) {
            return Some(function_type);
        }

        match self.current_kind() {
            Some(TokenKind::Identifier)
            | Some(TokenKind::NumberType)
            | Some(TokenKind::StringType)
            | Some(TokenKind::BooleanType)
            | Some(TokenKind::NeverType)
            | Some(TokenKind::UnknownType)
            | Some(TokenKind::Null)
            | Some(TokenKind::Undefined) => self.parse_named_or_generic_type(),
            Some(TokenKind::StringLiteral)
            | Some(TokenKind::NumberLiteral)
            | Some(TokenKind::True)
            | Some(TokenKind::False) => self.parse_literal_type().map(TypeExpr::Literal),
            Some(TokenKind::LeftBrace) => self.parse_record_type(),
            Some(TokenKind::LeftParen) => self.parse_grouped_type(),
            Some(_) => {
                self.push_diagnostic(ParseDiagnosticKind::InvalidType, self.current_span());
                None
            }
            None => {
                self.push_diagnostic(ParseDiagnosticKind::Expected("type"), self.eof_span());
                None
            }
        }
    }

    fn parse_named_or_generic_type(&mut self) -> Option<TypeExpr> {
        let name = self.parse_identifier()?;

        if self.current_kind() != Some(TokenKind::Less) {
            return Some(TypeExpr::Identifier(name));
        }

        self.bump();
        let mut args = Vec::new();

        while self.current_kind() != Some(TokenKind::Greater) {
            args.push(self.parse_type_expr()?);

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::Greater) {
                    break;
                }
            } else {
                break;
            }
        }

        let end = self.expect(TokenKind::Greater, "`>` to close generic type arguments")?;

        Some(TypeExpr::Generic {
            span: name.span.cover(end.span),
            name,
            args,
        })
    }

    fn parse_literal_type(&mut self) -> Option<LiteralType> {
        let token = self.bump()?;

        Some(match token.kind {
            TokenKind::StringLiteral => LiteralType::String {
                value: decode_string(token.span.slice(self.source.contents())),
                span: token.span,
            },
            TokenKind::NumberLiteral => LiteralType::Number {
                value: token
                    .span
                    .slice(self.source.contents())
                    .parse()
                    .unwrap_or_default(),
                span: token.span,
            },
            TokenKind::True => LiteralType::Boolean {
                value: true,
                span: token.span,
            },
            TokenKind::False => LiteralType::Boolean {
                value: false,
                span: token.span,
            },
            other => {
                self.push_diagnostic(ParseDiagnosticKind::UnexpectedToken(other), token.span);
                return None;
            }
        })
    }

    fn parse_record_type(&mut self) -> Option<TypeExpr> {
        let start = self.expect(TokenKind::LeftBrace, "`{` to start a record type")?;
        let mut fields = Vec::new();

        while self.current_kind() != Some(TokenKind::RightBrace) {
            let name = self.parse_identifier()?;
            self.expect(TokenKind::Colon, "`:` after a record type field name")?;
            let value = self.parse_type_expr()?;
            let span = name.span.cover(value.span());
            fields.push(RecordTypeField { name, value, span });

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightBrace) {
                    break;
                }
            } else {
                break;
            }
        }

        let end = self.expect(TokenKind::RightBrace, "`}` to close a record type")?;

        Some(TypeExpr::Record {
            fields,
            span: start.span.cover(end.span),
        })
    }

    fn parse_function_type(&mut self) -> Option<TypeExpr> {
        let start = self.expect(TokenKind::LeftParen, "`(` to start a function type")?;
        let mut params = Vec::new();

        while self.current_kind() != Some(TokenKind::RightParen) {
            let name = self.parse_identifier()?;
            self.expect(TokenKind::Colon, "`:` after a function type parameter name")?;
            let value = self.parse_type_expr()?;
            let span = name.span.cover(value.span());
            params.push(FunctionTypeParam { name, value, span });

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightParen) {
                    break;
                }
            } else {
                break;
            }
        }

        self.expect(TokenKind::RightParen, "`)` to close a function type")?;
        self.expect(TokenKind::Colon, "`:` before a function type return type")?;
        let return_type = self.parse_type_expr()?;

        Some(TypeExpr::Function {
            params,
            span: start.span.cover(return_type.span()),
            return_type: Box::new(return_type),
        })
    }

    fn parse_grouped_type(&mut self) -> Option<TypeExpr> {
        let start = self.expect(TokenKind::LeftParen, "`(` to start a grouped type")?;
        let inner = self.parse_type_expr()?;
        let end = self.expect(TokenKind::RightParen, "`)` to close a grouped type")?;

        Some(TypeExpr::Grouped {
            span: start.span.cover(end.span),
            inner: Box::new(inner),
        })
    }

    fn parse_expr(&mut self) -> Option<Expr> {
        self.parse_pipe_expr()
    }

    fn parse_pipe_expr(&mut self) -> Option<Expr> {
        let mut expr = self.parse_conditional_expr()?;

        while self.current_kind() == Some(TokenKind::Pipe) {
            self.bump();
            let right = self.parse_conditional_expr()?;
            let span = expr.span().cover(right.span());
            expr = Expr::Pipe {
                left: Box::new(expr),
                right: Box::new(right),
                span,
            };
        }

        Some(expr)
    }

    fn parse_conditional_expr(&mut self) -> Option<Expr> {
        match self.current_kind() {
            Some(TokenKind::If) => self.parse_if_expr(),
            Some(TokenKind::Match) => self.parse_match_expr(),
            Some(TokenKind::Try) => self.parse_try_expr(),
            _ => self.parse_logical_or_expr(),
        }
    }

    fn parse_if_expr(&mut self) -> Option<Expr> {
        let if_token = self.expect(TokenKind::If, "`if`")?;
        self.expect(TokenKind::LeftParen, "`(` after `if`")?;
        let condition = self.parse_expr()?;
        self.expect(TokenKind::RightParen, "`)` after an `if` condition")?;
        let then_branch = self.parse_block_expr()?;
        let else_branch = if self.current_kind() == Some(TokenKind::Else) {
            self.bump();
            Some(Box::new(if self.current_kind() == Some(TokenKind::If) {
                self.parse_if_expr()?
            } else {
                self.parse_block_expr()?
            }))
        } else {
            self.push_diagnostic(
                ParseDiagnosticKind::Expected("`else` after an `if` block"),
                self.current_span(),
            );
            None
        };

        let end_span = else_branch
            .as_ref()
            .map_or(then_branch.span(), |branch| branch.span());

        Some(Expr::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch,
            span: if_token.span.cover(end_span),
        })
    }

    fn parse_match_expr(&mut self) -> Option<Expr> {
        let match_token = self.expect(TokenKind::Match, "`match`")?;
        self.expect(TokenKind::LeftParen, "`(` after `match`")?;
        let value = self.parse_expr()?;
        self.expect(TokenKind::RightParen, "`)` after a `match` value")?;
        self.expect(TokenKind::LeftBrace, "`{` to start match arms")?;
        let mut arms = Vec::new();

        while self.current_kind() != Some(TokenKind::RightBrace) {
            let pattern = self.parse_pattern()?;
            self.expect(TokenKind::Arrow, "`=>` after a match pattern")?;
            let body = if self.current_kind() == Some(TokenKind::LeftBrace) {
                self.parse_block_expr()?
            } else {
                self.parse_expr()?
            };
            let span = pattern.span().cover(body.span());
            arms.push(MatchArm {
                pattern,
                body,
                span,
            });

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightBrace) {
                    break;
                }
            } else {
                break;
            }
        }

        let end = self.expect(TokenKind::RightBrace, "`}` to close a match expression")?;

        Some(Expr::Match {
            value: Box::new(value),
            arms,
            span: match_token.span.cover(end.span),
        })
    }

    fn parse_try_expr(&mut self) -> Option<Expr> {
        let try_token = self.expect(TokenKind::Try, "`try`")?;
        let body = self.parse_block_expr()?;
        self.expect(TokenKind::Catch, "`catch` after a try block")?;
        self.expect(TokenKind::LeftParen, "`(` after `catch`")?;
        let catch_pattern = self.parse_pattern()?;
        self.expect(TokenKind::RightParen, "`)` after a catch pattern")?;
        let catch_body = self.parse_block_expr()?;

        Some(Expr::Try {
            span: try_token.span.cover(catch_body.span()),
            body: Box::new(body),
            catch_pattern,
            catch_body: Box::new(catch_body),
        })
    }

    fn parse_logical_or_expr(&mut self) -> Option<Expr> {
        self.parse_left_associative_binary(
            Self::parse_logical_and_expr,
            &[(TokenKind::LogicalOr, BinaryOperator::LogicalOr)],
        )
    }

    fn parse_logical_and_expr(&mut self) -> Option<Expr> {
        self.parse_left_associative_binary(
            Self::parse_nullish_expr,
            &[(TokenKind::LogicalAnd, BinaryOperator::LogicalAnd)],
        )
    }

    fn parse_nullish_expr(&mut self) -> Option<Expr> {
        self.parse_left_associative_binary(
            Self::parse_equality_expr,
            &[(TokenKind::NullishCoalesce, BinaryOperator::NullishCoalesce)],
        )
    }

    fn parse_equality_expr(&mut self) -> Option<Expr> {
        self.parse_left_associative_binary(
            Self::parse_relational_expr,
            &[
                (TokenKind::StrictEqual, BinaryOperator::StrictEqual),
                (TokenKind::StrictNotEqual, BinaryOperator::StrictNotEqual),
            ],
        )
    }

    fn parse_relational_expr(&mut self) -> Option<Expr> {
        self.parse_left_associative_binary(
            Self::parse_additive_expr,
            &[
                (TokenKind::Less, BinaryOperator::Less),
                (TokenKind::LessEqual, BinaryOperator::LessEqual),
                (TokenKind::Greater, BinaryOperator::Greater),
                (TokenKind::GreaterEqual, BinaryOperator::GreaterEqual),
            ],
        )
    }

    fn parse_additive_expr(&mut self) -> Option<Expr> {
        self.parse_left_associative_binary(
            Self::parse_multiplicative_expr,
            &[
                (TokenKind::Plus, BinaryOperator::Add),
                (TokenKind::Minus, BinaryOperator::Subtract),
            ],
        )
    }

    fn parse_multiplicative_expr(&mut self) -> Option<Expr> {
        self.parse_left_associative_binary(
            Self::parse_unary_expr,
            &[
                (TokenKind::Star, BinaryOperator::Multiply),
                (TokenKind::Slash, BinaryOperator::Divide),
                (TokenKind::Percent, BinaryOperator::Modulo),
            ],
        )
    }

    fn parse_left_associative_binary(
        &mut self,
        next: fn(&mut Self) -> Option<Expr>,
        operators: &[(TokenKind, BinaryOperator)],
    ) -> Option<Expr> {
        let mut expr = next(self)?;

        while let Some((_, operator)) = operators
            .iter()
            .find(|(kind, _)| Some(*kind) == self.current_kind())
        {
            self.bump();
            let right = next(self)?;
            let span = expr.span().cover(right.span());
            expr = Expr::Binary {
                operator: *operator,
                left: Box::new(expr),
                right: Box::new(right),
                span,
            };
        }

        Some(expr)
    }

    fn parse_unary_expr(&mut self) -> Option<Expr> {
        let operator = match self.current_kind() {
            Some(TokenKind::Bang) => Some(UnaryOperator::Not),
            Some(TokenKind::Minus) => Some(UnaryOperator::Negate),
            Some(TokenKind::Plus) => Some(UnaryOperator::Positive),
            Some(TokenKind::Defer) => Some(UnaryOperator::Defer),
            _ => None,
        };

        if let Some(operator) = operator {
            let token = self.bump()?;
            let operand = self.parse_unary_expr()?;
            return Some(Expr::Unary {
                operator,
                span: token.span.cover(operand.span()),
                operand: Box::new(operand),
            });
        }

        self.parse_postfix_expr()
    }

    fn parse_postfix_expr(&mut self) -> Option<Expr> {
        let mut expr = self.parse_primary_expr()?;

        loop {
            match self.current_kind() {
                Some(TokenKind::LeftParen) => {
                    let start = expr.span();
                    self.bump();
                    let mut args = Vec::new();

                    while self.current_kind() != Some(TokenKind::RightParen) {
                        args.push(self.parse_expr()?);

                        if self.current_kind() == Some(TokenKind::Comma) {
                            self.bump();
                            if self.current_kind() == Some(TokenKind::RightParen) {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    let end = self.expect(TokenKind::RightParen, "`)` to close a call")?;
                    expr = Expr::Call {
                        callee: Box::new(expr),
                        args,
                        span: start.cover(end.span),
                    };
                }
                Some(TokenKind::Dot) => {
                    let start = expr.span();
                    self.bump();
                    let property = self.parse_property_identifier()?;
                    let span = start.cover(property.span);
                    expr = Expr::Member {
                        object: Box::new(expr),
                        property,
                        span,
                    };
                }
                Some(TokenKind::LeftBracket) => {
                    let start = expr.span();
                    self.bump();
                    let index = self.parse_expr()?;
                    let end =
                        self.expect(TokenKind::RightBracket, "`]` to close an index access")?;
                    expr = Expr::Index {
                        object: Box::new(expr),
                        index: Box::new(index),
                        span: start.cover(end.span),
                    };
                }
                _ => break,
            }
        }

        Some(expr)
    }

    fn parse_primary_expr(&mut self) -> Option<Expr> {
        if let Some(function) = self.parse_tentatively(|parser| parser.parse_function_expr(false)) {
            return Some(function);
        }

        if let Some(generator) = self.parse_tentatively(|parser| parser.parse_function_expr(true)) {
            return Some(generator);
        }

        if let Some(record) = self.parse_tentatively(|parser| parser.parse_record_literal()) {
            return Some(record);
        }

        match self.current_kind() {
            Some(TokenKind::StringLiteral)
            | Some(TokenKind::NumberLiteral)
            | Some(TokenKind::True)
            | Some(TokenKind::False)
            | Some(TokenKind::Null)
            | Some(TokenKind::Undefined) => self.parse_literal_expr(),
            Some(TokenKind::Identifier)
            | Some(TokenKind::NumberType)
            | Some(TokenKind::StringType)
            | Some(TokenKind::BooleanType)
            | Some(TokenKind::NeverType)
            | Some(TokenKind::UnknownType) => self.parse_identifier().map(Expr::Identifier),
            Some(TokenKind::LeftBracket) => self.parse_array_literal(),
            Some(TokenKind::LeftBrace) => self.parse_block_expr(),
            Some(TokenKind::LeftParen) => self.parse_grouped_expr(),
            Some(TokenKind::Throw) => self.parse_throw_expr(),
            Some(TokenKind::Yield) => self.parse_yield_expr(),
            Some(kind) => {
                self.push_diagnostic(
                    ParseDiagnosticKind::UnexpectedToken(kind),
                    self.current_span(),
                );
                None
            }
            None => {
                self.push_diagnostic(ParseDiagnosticKind::Expected("expression"), self.eof_span());
                None
            }
        }
    }

    fn parse_literal_expr(&mut self) -> Option<Expr> {
        let token = self.bump()?;

        Some(match token.kind {
            TokenKind::StringLiteral => Expr::StringLiteral {
                value: decode_string(token.span.slice(self.source.contents())),
                span: token.span,
            },
            TokenKind::NumberLiteral => Expr::NumberLiteral {
                value: token
                    .span
                    .slice(self.source.contents())
                    .parse()
                    .unwrap_or_default(),
                span: token.span,
            },
            TokenKind::True => Expr::BooleanLiteral {
                value: true,
                span: token.span,
            },
            TokenKind::False => Expr::BooleanLiteral {
                value: false,
                span: token.span,
            },
            TokenKind::Null => Expr::Null { span: token.span },
            TokenKind::Undefined => Expr::Undefined { span: token.span },
            other => {
                self.push_diagnostic(ParseDiagnosticKind::UnexpectedToken(other), token.span);
                return None;
            }
        })
    }

    fn parse_array_literal(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::LeftBracket, "`[` to start an array literal")?;
        let mut items = Vec::new();

        while self.current_kind() != Some(TokenKind::RightBracket) {
            items.push(self.parse_expr()?);

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightBracket) {
                    break;
                }
            } else {
                break;
            }
        }

        let end = self.expect(TokenKind::RightBracket, "`]` to close an array literal")?;

        Some(Expr::Array {
            items,
            span: start.span.cover(end.span),
        })
    }

    fn parse_record_literal(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::LeftBrace, "`{` to start a record literal")?;
        let mut fields = Vec::new();

        while self.current_kind() != Some(TokenKind::RightBrace) {
            let name = self.parse_identifier()?;
            self.expect(TokenKind::Colon, "`:` after a record literal field name")?;
            let value = self.parse_expr()?;
            let span = name.span.cover(value.span());
            fields.push(RecordField { name, value, span });

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightBrace) {
                    break;
                }
            } else {
                break;
            }
        }

        let end = self.expect(TokenKind::RightBrace, "`}` to close a record literal")?;

        Some(Expr::Record {
            fields,
            span: start.span.cover(end.span),
        })
    }

    fn parse_block_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::LeftBrace, "`{` to start a block")?;
        let mut items = Vec::new();

        while self.current_kind() != Some(TokenKind::RightBrace) {
            if let Some(binding) = self.parse_tentatively(|parser| parser.parse_binding_decl()) {
                items.push(BlockItem::Binding(binding));
                continue;
            }

            items.push(BlockItem::Expr(self.parse_expr()?));
        }

        let end = self.expect(TokenKind::RightBrace, "`}` to close a block")?;

        Some(Expr::Block {
            items,
            span: start.span.cover(end.span),
        })
    }

    fn parse_grouped_expr(&mut self) -> Option<Expr> {
        let start = self.expect(TokenKind::LeftParen, "`(` to start a grouped expression")?;
        let inner = self.parse_expr()?;
        let end = self.expect(TokenKind::RightParen, "`)` to close a grouped expression")?;

        Some(Expr::Grouped {
            inner: Box::new(inner),
            span: start.span.cover(end.span),
        })
    }

    fn parse_throw_expr(&mut self) -> Option<Expr> {
        let token = self.expect(TokenKind::Throw, "`throw`")?;
        let value = self.parse_expr()?;

        Some(Expr::Throw {
            span: token.span.cover(value.span()),
            value: Box::new(value),
        })
    }

    fn parse_yield_expr(&mut self) -> Option<Expr> {
        let token = self.expect(TokenKind::Yield, "`yield`")?;
        let value = self.parse_expr()?;

        Some(Expr::Yield {
            span: token.span.cover(value.span()),
            value: Box::new(value),
        })
    }

    fn parse_function_expr(&mut self, is_generator: bool) -> Option<Expr> {
        let start = if is_generator {
            self.expect(TokenKind::Star, "`*` to start a generator arrow")?
                .span
        } else {
            self.current()?.span
        };

        self.expect(
            TokenKind::LeftParen,
            "`(` to start a function parameter list",
        )?;
        let mut parameters = Vec::new();

        while self.current_kind() != Some(TokenKind::RightParen) {
            let parameter_start = self.current_span();
            let pattern = self.parse_pattern()?;
            let type_annotation = if self.current_kind() == Some(TokenKind::Colon) {
                self.bump();
                Some(self.parse_type_expr()?)
            } else {
                None
            };
            let parameter_end = type_annotation
                .as_ref()
                .map_or(pattern.span(), TypeExpr::span);
            parameters.push(Parameter {
                span: parameter_start.cover(parameter_end),
                pattern,
                type_annotation,
            });

            if self.current_kind() == Some(TokenKind::Comma) {
                self.bump();
                if self.current_kind() == Some(TokenKind::RightParen) {
                    break;
                }
            } else {
                break;
            }
        }

        self.expect(
            TokenKind::RightParen,
            "`)` to close a function parameter list",
        )?;
        let return_type = if self.current_kind() == Some(TokenKind::Colon) {
            self.bump();
            Some(self.parse_type_expr()?)
        } else {
            None
        };
        self.expect(TokenKind::Arrow, "`=>` after a function head")?;
        let body = if self.current_kind() == Some(TokenKind::LeftBrace) {
            self.parse_block_expr()?
        } else {
            self.parse_expr()?
        };

        Some(Expr::Function {
            parameters,
            return_type,
            body: Box::new(body.clone()),
            is_generator,
            span: start.cover(body.span()),
        })
    }

    fn parse_identifier(&mut self) -> Option<Identifier> {
        let token = self.bump()?;
        if !is_identifier_like(token.kind) {
            self.push_diagnostic(ParseDiagnosticKind::Expected("identifier"), token.span);
            return None;
        }

        Some(Identifier {
            name: token.span.slice(self.source.contents()).to_owned(),
            span: token.span,
        })
    }

    fn parse_property_identifier(&mut self) -> Option<Identifier> {
        let token = self.bump()?;
        if !is_property_name(token.kind) {
            self.push_diagnostic(ParseDiagnosticKind::Expected("identifier"), token.span);
            return None;
        }

        Some(Identifier {
            name: token.span.slice(self.source.contents()).to_owned(),
            span: token.span,
        })
    }

    fn parse_tentatively<T>(&mut self, f: impl FnOnce(&mut Self) -> Option<T>) -> Option<T> {
        let checkpoint = self.checkpoint();
        let result = f(self);

        if result.is_some() {
            result
        } else {
            self.rewind(checkpoint);
            None
        }
    }

    fn checkpoint(&self) -> (usize, usize) {
        (self.index, self.diagnostics.len())
    }

    fn rewind(&mut self, checkpoint: (usize, usize)) {
        self.index = checkpoint.0;
        self.diagnostics.truncate(checkpoint.1);
    }

    fn recover_module_item(&mut self) {
        if self.is_at_end() {
            return;
        }

        self.index += 1;

        while !self.is_at_end() {
            match self.current_kind() {
                Some(TokenKind::Import)
                | Some(TokenKind::Export)
                | Some(TokenKind::Type)
                | Some(TokenKind::Identifier)
                | Some(TokenKind::LeftBrace)
                | Some(TokenKind::LeftBracket) => break,
                _ => self.index += 1,
            }
        }
    }

    fn report_adjacent_expression_token(&mut self, expr_span: Span) {
        let Some(token) = self.current() else {
            return;
        };

        if token.span.start() != expr_span.end() {
            return;
        }

        if matches!(
            token.kind,
            TokenKind::Comma
                | TokenKind::RightParen
                | TokenKind::RightBrace
                | TokenKind::RightBracket
        ) {
            return;
        }

        self.push_diagnostic(
            ParseDiagnosticKind::Expected("an operator or delimiter after the expression"),
            token.span,
        );
    }

    fn expect(&mut self, kind: TokenKind, expected: &'static str) -> Option<Token> {
        match self.current().copied() {
            Some(token) if token.kind == kind => {
                self.index += 1;
                Some(token)
            }
            Some(token) => {
                self.push_diagnostic(ParseDiagnosticKind::Expected(expected), token.span);
                None
            }
            None => {
                self.push_diagnostic(ParseDiagnosticKind::Expected(expected), self.eof_span());
                None
            }
        }
    }

    fn push_diagnostic(&mut self, kind: ParseDiagnosticKind, span: Span) {
        self.diagnostics.push(ParseDiagnostic { kind, span });
    }

    fn current(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn current_kind(&self) -> Option<TokenKind> {
        self.current().map(|token| token.kind)
    }

    fn current_span(&self) -> Span {
        self.current().map_or(self.eof_span(), |token| token.span)
    }

    fn nth_kind(&self, offset: usize) -> Option<TokenKind> {
        self.tokens.get(self.index + offset).map(|token| token.kind)
    }

    fn bump(&mut self) -> Option<Token> {
        let token = self.current().copied()?;
        self.index += 1;
        Some(token)
    }

    fn eof_span(&self) -> Span {
        Span::new(self.source.contents().len(), self.source.contents().len())
    }

    fn is_at_end(&self) -> bool {
        self.index >= self.tokens.len()
    }
}

fn is_identifier_like(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier
            | TokenKind::NumberType
            | TokenKind::StringType
            | TokenKind::BooleanType
            | TokenKind::Null
            | TokenKind::Undefined
            | TokenKind::NeverType
            | TokenKind::UnknownType
    )
}

fn is_property_name(kind: TokenKind) -> bool {
    is_identifier_like(kind)
        || matches!(
            kind,
            TokenKind::Import
                | TokenKind::From
                | TokenKind::Export
                | TokenKind::Type
                | TokenKind::If
                | TokenKind::Else
                | TokenKind::Match
                | TokenKind::Try
                | TokenKind::Catch
                | TokenKind::Throw
                | TokenKind::Defer
                | TokenKind::Yield
                | TokenKind::True
                | TokenKind::False
        )
}

fn is_trivia(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Whitespace | TokenKind::LineComment | TokenKind::BlockComment
    )
}

fn decode_string(raw: &str) -> String {
    let mut chars = raw.chars();
    let quote = chars.next().unwrap_or('"');
    let mut decoded = String::new();
    let mut escaping = false;

    for ch in chars {
        if escaping {
            decoded.push(match ch {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                '0' => '\0',
                '\\' => '\\',
                '\'' => '\'',
                '"' => '"',
                other => other,
            });
            escaping = false;
            continue;
        }

        if ch == '\\' {
            escaping = true;
            continue;
        }

        if ch == quote {
            break;
        }

        decoded.push(ch);
    }

    decoded
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use fscript_ast::Module;
    use fscript_lexer::lex;
    use fscript_test_support::example_source_paths;
    use insta::assert_snapshot;
    use proptest::prelude::*;

    use super::{ModuleItem, ParseDiagnostic, ParseDiagnosticKind, parse_module};

    fn parse(text: &str) -> super::ParsedModule {
        let source = fscript_source::SourceFile::new(Utf8PathBuf::from("test.fs"), text.to_owned());
        let lexed = lex(&source);
        assert!(
            lexed.diagnostics.is_empty(),
            "lexer diagnostics: {:?}",
            lexed.diagnostics
        );
        parse_module(&source, &lexed.tokens)
    }

    #[test]
    fn parses_imports_type_declarations_and_bindings() {
        let parsed = parse(
            "import Array from 'std:array'\n\
             type User = { name: String }\n\
             user = { name: 'Ada' }",
        );

        assert!(parsed.diagnostics.is_empty());
        assert_eq!(parsed.module.items.len(), 3);
        assert!(matches!(parsed.module.items[0], ModuleItem::Import(_)));
        assert!(matches!(parsed.module.items[1], ModuleItem::Type(_)));
        assert!(matches!(parsed.module.items[2], ModuleItem::Binding(_)));
    }

    #[test]
    fn parses_functions_blocks_and_pipe_expressions() {
        let parsed = parse(
            "import Array from 'std:array'\n\
             addOne = (value: Number): Number => value + 1\n\
             result = [1, 2, 3] |> Array.map(addOne)",
        );

        assert!(parsed.diagnostics.is_empty());
        assert_eq!(parsed.module.items.len(), 3);
    }

    #[test]
    fn parses_match_if_and_generator_expressions() {
        let parsed = parse(concat!(
            "counter = *(start: Number, end: Number): Sequence<Number> => {\n",
            "  if (start < end) {\n",
            "    yield start\n",
            "  } else {\n",
            "    yield end\n",
            "  }\n",
            "}\n",
            "display = (user: User): String => match (user) {\n",
            "  { tag: 'guest' } => 'Guest',\n",
            "  { tag: 'member', name } => name,\n",
            "}",
        ));

        assert!(parsed.diagnostics.is_empty());
        assert_eq!(parsed.module.items.len(), 2);
    }

    #[test]
    fn reports_missing_assignment_after_binding_pattern() {
        let parsed = parse("message 'hello'");

        assert_eq!(parsed.diagnostics.len(), 1);
        assert_eq!(
            parsed.diagnostics[0].kind,
            ParseDiagnosticKind::Expected("`=` after a binding pattern")
        );
    }

    #[test]
    fn reports_missing_else_after_if_expression() {
        let parsed = parse("answer = if (true) { 1 }");

        assert_eq!(parsed.diagnostics.len(), 1);
        assert_eq!(
            parsed.diagnostics[0].kind,
            ParseDiagnosticKind::Expected("`else` after an `if` block")
        );
    }

    #[test]
    fn snapshots_representative_module_and_recovery_diagnostics() {
        let parsed = parse(
            "import Array from 'std:array'\n\
             addOne = (value: Number): Number => value + 1\n\
             result = [1, 2, 3] |> Array.map(addOne)\n\
             broken 'value'\n",
        );

        assert_snapshot!(
            "representative_parsed_module",
            format_parsed_module(&parsed.module, &parsed.diagnostics)
        );
    }

    #[test]
    fn all_examples_parse_without_diagnostics() {
        for path in example_source_paths() {
            let source =
                fscript_source::SourceFile::load(&path).expect("example source should load");
            let lexed = lex(&source);
            assert!(
                lexed.diagnostics.is_empty(),
                "lexing failed for {path}: {:?}",
                lexed.diagnostics
            );

            let parsed = parse_module(&source, &lexed.tokens);
            assert!(
                parsed.diagnostics.is_empty(),
                "parsing failed for {path}: {:?}",
                parsed.diagnostics
            );
        }
    }

    proptest! {
        #[test]
        fn generated_binding_modules_parse_without_diagnostics((module_text, expected_items) in module_strategy()) {
            let source = fscript_source::SourceFile::new(Utf8PathBuf::from("test.fs"), module_text);
            let lexed = lex(&source);

            prop_assert!(
                lexed.diagnostics.is_empty(),
                "lexer diagnostics: {:?}",
                lexed.diagnostics
            );

            let parsed = parse_module(&source, &lexed.tokens);
            prop_assert!(
                parsed.diagnostics.is_empty(),
                "parser diagnostics: {:?}",
                parsed.diagnostics
            );
            prop_assert_eq!(parsed.module.items.len(), expected_items);
        }

        #[test]
        fn module_ast_is_stable_when_only_top_level_trivia_changes(items in proptest::collection::vec(simple_binding_decl_strategy(), 1..6)) {
            let compact = items.join("\n");
            let with_trivia = items.join("\n// separator\n\n");

            let compact_parsed = parse(&compact);
            let trivia_parsed = parse(&with_trivia);

            prop_assert!(compact_parsed.diagnostics.is_empty());
            prop_assert!(trivia_parsed.diagnostics.is_empty());
            prop_assert_eq!(compact_parsed.module.items.len(), trivia_parsed.module.items.len());
            prop_assert_eq!(
                compact_parsed
                    .module
                    .items
                    .iter()
                    .map(module_item_kind)
                    .collect::<Vec<_>>(),
                trivia_parsed
                    .module
                    .items
                    .iter()
                    .map(module_item_kind)
                    .collect::<Vec<_>>(),
            );
        }
    }

    fn module_strategy() -> impl Strategy<Value = (String, usize)> {
        proptest::collection::vec(simple_binding_decl_strategy(), 1..8)
            .prop_map(|items| (items.join("\n"), items.len()))
    }

    fn simple_binding_decl_strategy() -> impl Strategy<Value = String> {
        (identifier_strategy(), expr_strategy())
            .prop_map(|(pattern, expr)| format!("{pattern} = {expr}"))
    }

    fn expr_strategy() -> impl Strategy<Value = String> {
        let leaf = prop_oneof![identifier_strategy(), literal_strategy(),];

        leaf.prop_recursive(4, 32, 4, |inner| {
            prop_oneof![
                (unary_operator_strategy(), inner.clone())
                    .prop_map(|(operator, expr)| format!("{operator}{expr}")),
                (inner.clone(), binary_operator_strategy(), inner.clone(),)
                    .prop_map(|(left, operator, right)| format!("({left} {operator} {right})")),
                proptest::collection::vec(inner.clone(), 1..4)
                    .prop_map(|items| format!("[{}]", items.join(", "))),
                proptest::collection::vec((identifier_strategy(), inner.clone()), 1..4,).prop_map(
                    |fields| {
                        let fields = fields
                            .into_iter()
                            .map(|(name, value)| format!("{name}: {value}"))
                            .collect::<Vec<_>>();
                        format!("{{{}}}", fields.join(", "))
                    }
                ),
            ]
        })
    }

    fn identifier_strategy() -> impl Strategy<Value = String> {
        (
            prop_oneof![
                Just('_'),
                proptest::char::range('a', 'z'),
                proptest::char::range('A', 'Z')
            ],
            proptest::collection::vec(
                prop_oneof![
                    Just('_'),
                    proptest::char::range('a', 'z'),
                    proptest::char::range('A', 'Z'),
                    proptest::char::range('0', '9')
                ],
                0..8,
            ),
        )
            .prop_map(|(head, tail)| std::iter::once(head).chain(tail).collect())
            .prop_filter(
                "reserved keywords are excluded from generated identifiers",
                |identifier: &String| {
                    !matches!(
                        identifier.as_str(),
                        "import"
                            | "from"
                            | "export"
                            | "type"
                            | "if"
                            | "else"
                            | "match"
                            | "try"
                            | "catch"
                            | "throw"
                            | "defer"
                            | "yield"
                            | "true"
                            | "false"
                            | "Number"
                            | "String"
                            | "Boolean"
                            | "Null"
                            | "Undefined"
                            | "Never"
                            | "Unknown"
                    )
                },
            )
    }

    fn literal_strategy() -> impl Strategy<Value = String> {
        prop_oneof![
            (0_u16..10_000).prop_map(|value| value.to_string()),
            safe_string_contents_strategy().prop_map(|contents| format!("'{contents}'")),
            Just(String::from("true")),
            Just(String::from("false")),
            Just(String::from("null")),
            Just(String::from("undefined")),
        ]
    }

    fn safe_string_contents_strategy() -> impl Strategy<Value = String> {
        proptest::collection::vec(
            prop_oneof![
                proptest::char::range('a', 'z'),
                proptest::char::range('A', 'Z'),
                proptest::char::range('0', '9'),
                Just(' '),
                Just('_'),
                Just('-'),
            ],
            0..8,
        )
        .prop_map(|chars: Vec<char>| chars.into_iter().collect())
    }

    fn unary_operator_strategy() -> impl Strategy<Value = &'static str> {
        prop_oneof![Just("!"), Just("-"), Just("+"), Just("defer "),]
    }

    fn binary_operator_strategy() -> impl Strategy<Value = &'static str> {
        prop_oneof![
            Just("+"),
            Just("-"),
            Just("*"),
            Just("/"),
            Just("%"),
            Just("==="),
            Just("!=="),
            Just("<"),
            Just("<="),
            Just(">"),
            Just(">="),
            Just("&&"),
            Just("||"),
            Just("??"),
            Just("|>"),
        ]
    }

    fn format_parsed_module(module: &Module, diagnostics: &[ParseDiagnostic]) -> String {
        let mut output = format!("{module:#?}\n");
        output.push_str("diagnostics:\n");
        for diagnostic in diagnostics {
            output.push_str(&format!(
                "- {:?} {:?}\n",
                diagnostic.kind,
                (diagnostic.span.start(), diagnostic.span.end())
            ));
        }
        output
    }

    fn module_item_kind(item: &ModuleItem) -> &'static str {
        match item {
            ModuleItem::Import(_) => "import",
            ModuleItem::Type(_) | ModuleItem::ExportType(_) => "type",
            ModuleItem::Binding(_) | ModuleItem::ExportBinding(_) => "binding",
        }
    }
}
