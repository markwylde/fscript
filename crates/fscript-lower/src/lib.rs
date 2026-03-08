//! Lowering from AST into resolved HIR.

use std::collections::BTreeMap;

use fscript_ast as ast;
use fscript_hir as hir;
use fscript_ir as ir;
use fscript_source::Span;
use thiserror::Error;

/// Lowers a parsed module into the first semantic HIR.
pub fn lower_module(module: &ast::Module) -> Result<hir::Module, LowerError> {
    Lowerer::new().lower_module(module)
}

/// Lowers the resolved HIR into the first shared executable IR.
#[must_use]
pub fn lower_to_ir(module: &hir::Module) -> ir::Module {
    ir::Module {
        items: module
            .items
            .iter()
            .filter_map(lower_module_item_to_ir)
            .collect(),
        exports: module
            .items
            .iter()
            .filter_map(|item| match item {
                hir::ModuleItem::Binding(binding) if binding.is_exported => {
                    collect_exported_names(&binding.pattern)
                }
                _ => None,
            })
            .flatten()
            .collect(),
    }
}

/// Name-resolution and lowering failures.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LowerError {
    #[error("unknown identifier `{name}`")]
    UnknownIdentifier { name: String, span: Span },
    #[error("unknown type `{name}`")]
    UnknownType { name: String, span: Span },
    #[error("binding `{name}` is already defined in this scope")]
    DuplicateBinding { name: String, span: Span },
    #[error("type `{name}` is already defined in this scope")]
    DuplicateType { name: String, span: Span },
}

impl LowerError {
    /// Returns the source span for the diagnostic.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::UnknownIdentifier { span, .. }
            | Self::UnknownType { span, .. }
            | Self::DuplicateBinding { span, .. }
            | Self::DuplicateType { span, .. } => *span,
        }
    }
}

struct Lowerer {
    value_scopes: Vec<BTreeMap<String, hir::BindingName>>,
    type_scopes: Vec<BTreeMap<String, ResolvedTypeName>>,
    next_binding_id: u32,
    next_type_id: u32,
    next_type_param_id: u32,
}

#[derive(Clone)]
enum ResolvedTypeName {
    Reference(hir::TypeReference),
    AliasDeclaration(hir::TypeName),
}

impl Lowerer {
    fn new() -> Self {
        let mut lowerer = Self {
            value_scopes: vec![BTreeMap::new()],
            type_scopes: vec![BTreeMap::new()],
            next_binding_id: 0,
            next_type_id: 0,
            next_type_param_id: 0,
        };

        lowerer.install_builtin_types();
        lowerer
    }

    fn lower_module(&mut self, module: &ast::Module) -> Result<hir::Module, LowerError> {
        let items = module
            .items
            .iter()
            .map(|item| self.lower_module_item(item))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(hir::Module { items })
    }

    fn lower_module_item(&mut self, item: &ast::ModuleItem) -> Result<hir::ModuleItem, LowerError> {
        match item {
            ast::ModuleItem::Import(import) => {
                self.lower_import_decl(import).map(hir::ModuleItem::Import)
            }
            ast::ModuleItem::Type(ty) => self.lower_type_decl(ty, false).map(hir::ModuleItem::Type),
            ast::ModuleItem::ExportType(ty) => {
                self.lower_type_decl(ty, true).map(hir::ModuleItem::Type)
            }
            ast::ModuleItem::Binding(binding) => self
                .lower_binding_decl(binding, false)
                .map(hir::ModuleItem::Binding),
            ast::ModuleItem::ExportBinding(binding) => self
                .lower_binding_decl(binding, true)
                .map(hir::ModuleItem::Binding),
        }
    }

    fn lower_import_decl(
        &mut self,
        import: &ast::ImportDecl,
    ) -> Result<hir::ImportDecl, LowerError> {
        let clause = match &import.clause {
            ast::ImportClause::Default(identifier) => {
                hir::ImportClause::Default(self.declare_binding(&identifier.name, identifier.span)?)
            }
            ast::ImportClause::Named(names) => hir::ImportClause::Named(
                names
                    .iter()
                    .map(|name| self.declare_binding(&name.name, name.span))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
        };

        Ok(hir::ImportDecl {
            clause,
            source: import.source.clone(),
            source_span: import.source_span,
            span: import.span,
        })
    }

    fn lower_type_decl(
        &mut self,
        declaration: &ast::TypeDecl,
        is_exported: bool,
    ) -> Result<hir::TypeDecl, LowerError> {
        let name = self.declare_type_alias(&declaration.name.name, declaration.name.span)?;

        self.push_type_scope();
        let mut type_params = Vec::new();
        for param in &declaration.type_params {
            let declared = self.declare_type_param(&param.name, param.span)?;
            type_params.push(declared);
        }
        let value = self.lower_type_expr(&declaration.value)?;
        self.pop_type_scope();

        Ok(hir::TypeDecl {
            name,
            type_params,
            value,
            is_exported,
            span: declaration.span,
        })
    }

    fn lower_binding_decl(
        &mut self,
        binding: &ast::BindingDecl,
        is_exported: bool,
    ) -> Result<hir::BindingDecl, LowerError> {
        let value = self.lower_expr(&binding.value)?;
        let pattern = self.declare_pattern(&binding.pattern)?;

        Ok(hir::BindingDecl {
            pattern,
            value,
            is_exported,
            span: binding.span,
        })
    }

    fn lower_expr(&mut self, expr: &ast::Expr) -> Result<hir::Expr, LowerError> {
        match expr {
            ast::Expr::StringLiteral { value, span } => Ok(hir::Expr::StringLiteral {
                value: value.clone(),
                span: *span,
            }),
            ast::Expr::NumberLiteral { value, span } => Ok(hir::Expr::NumberLiteral {
                value: *value,
                span: *span,
            }),
            ast::Expr::BooleanLiteral { value, span } => Ok(hir::Expr::BooleanLiteral {
                value: *value,
                span: *span,
            }),
            ast::Expr::Null { span } => Ok(hir::Expr::Null { span: *span }),
            ast::Expr::Undefined { span } => Ok(hir::Expr::Undefined { span: *span }),
            ast::Expr::Identifier(identifier) => Ok(hir::Expr::Identifier(
                self.resolve_identifier(&identifier.name, identifier.span)?,
            )),
            ast::Expr::Record { fields, span } => Ok(hir::Expr::Record {
                fields: fields
                    .iter()
                    .map(|field| {
                        Ok(hir::RecordField {
                            name: field.name.name.clone(),
                            span: field.span,
                            value: self.lower_expr(&field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::Expr::Array { items, span } => Ok(hir::Expr::Array {
                items: items
                    .iter()
                    .map(|item| self.lower_expr(item))
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::Expr::Function {
                parameters,
                return_type,
                body,
                is_generator,
                span,
            } => self.lower_function_expr(
                parameters,
                return_type.as_ref(),
                body,
                *is_generator,
                *span,
            ),
            ast::Expr::Block { items, span } => self.lower_block_expr(items, *span),
            ast::Expr::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => Ok(hir::Expr::If {
                condition: Box::new(self.lower_expr(condition)?),
                then_branch: Box::new(self.lower_expr(then_branch)?),
                else_branch: else_branch
                    .as_ref()
                    .map(|branch| self.lower_expr(branch))
                    .transpose()?
                    .map(Box::new),
                span: *span,
            }),
            ast::Expr::Match { value, arms, span } => {
                let value = Box::new(self.lower_expr(value)?);
                let arms = arms
                    .iter()
                    .map(|arm| self.lower_match_arm(arm))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(hir::Expr::Match {
                    value,
                    arms,
                    span: *span,
                })
            }
            ast::Expr::Try {
                body,
                catch_pattern,
                catch_body,
                span,
            } => {
                let body = Box::new(self.lower_expr(body)?);
                self.push_value_scope();
                let catch_pattern = self.declare_pattern(catch_pattern)?;
                let catch_body = Box::new(self.lower_expr(catch_body)?);
                self.pop_value_scope();

                Ok(hir::Expr::Try {
                    body,
                    catch_pattern,
                    catch_body,
                    span: *span,
                })
            }
            ast::Expr::Throw { value, span } => Ok(hir::Expr::Throw {
                value: Box::new(self.lower_expr(value)?),
                span: *span,
            }),
            ast::Expr::Yield { value, span } => Ok(hir::Expr::Yield {
                value: Box::new(self.lower_expr(value)?),
                span: *span,
            }),
            ast::Expr::Unary {
                operator,
                operand,
                span,
            } => Ok(hir::Expr::Unary {
                operator: lower_unary_operator(*operator),
                operand: Box::new(self.lower_expr(operand)?),
                span: *span,
            }),
            ast::Expr::Binary {
                operator,
                left,
                right,
                span,
            } => Ok(hir::Expr::Binary {
                operator: lower_binary_operator(*operator),
                left: Box::new(self.lower_expr(left)?),
                right: Box::new(self.lower_expr(right)?),
                span: *span,
            }),
            ast::Expr::Pipe { left, right, span } => self.lower_pipe_expr(left, right, *span),
            ast::Expr::Call { callee, args, span } => Ok(hir::Expr::Call {
                callee: Box::new(self.lower_expr(callee)?),
                args: args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::Expr::Member {
                object,
                property,
                span,
            } => Ok(hir::Expr::Member {
                object: Box::new(self.lower_expr(object)?),
                property: property.name.clone(),
                span: *span,
            }),
            ast::Expr::Index {
                object,
                index,
                span,
            } => Ok(hir::Expr::Index {
                object: Box::new(self.lower_expr(object)?),
                index: Box::new(self.lower_expr(index)?),
                span: *span,
            }),
            ast::Expr::Grouped { inner, .. } => self.lower_expr(inner),
        }
    }

    fn lower_function_expr(
        &mut self,
        parameters: &[ast::Parameter],
        return_type: Option<&ast::TypeExpr>,
        body: &ast::Expr,
        is_generator: bool,
        span: Span,
    ) -> Result<hir::Expr, LowerError> {
        self.push_value_scope();
        let parameters = parameters
            .iter()
            .map(|parameter| {
                let type_annotation = parameter
                    .type_annotation
                    .as_ref()
                    .map(|annotation| self.lower_type_expr(annotation))
                    .transpose()?;
                let pattern = self.declare_pattern(&parameter.pattern)?;
                Ok(hir::Parameter {
                    pattern,
                    type_annotation,
                    span: parameter.span,
                })
            })
            .collect::<Result<Vec<_>, LowerError>>()?;
        let return_type = return_type
            .map(|annotation| self.lower_type_expr(annotation))
            .transpose()?;
        let body = Box::new(self.lower_expr(body)?);
        self.pop_value_scope();

        Ok(hir::Expr::Function {
            parameters,
            return_type,
            body,
            is_generator,
            span,
        })
    }

    fn lower_block_expr(
        &mut self,
        items: &[ast::BlockItem],
        span: Span,
    ) -> Result<hir::Expr, LowerError> {
        self.push_value_scope();
        let items = items
            .iter()
            .map(|item| match item {
                ast::BlockItem::Binding(binding) => self
                    .lower_binding_decl(binding, false)
                    .map(hir::BlockItem::Binding),
                ast::BlockItem::Expr(expr) => self.lower_expr(expr).map(hir::BlockItem::Expr),
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.pop_value_scope();

        Ok(hir::Expr::Block { items, span })
    }

    fn lower_match_arm(&mut self, arm: &ast::MatchArm) -> Result<hir::MatchArm, LowerError> {
        self.push_value_scope();
        let pattern = self.declare_pattern(&arm.pattern)?;
        let body = self.lower_expr(&arm.body)?;
        self.pop_value_scope();

        Ok(hir::MatchArm {
            pattern,
            body,
            span: arm.span,
        })
    }

    fn lower_pipe_expr(
        &mut self,
        left: &ast::Expr,
        right: &ast::Expr,
        span: Span,
    ) -> Result<hir::Expr, LowerError> {
        let left = self.lower_expr(left)?;
        match right {
            ast::Expr::Call { callee, args, .. } => {
                let callee = Box::new(self.lower_expr(callee)?);
                let mut lowered_args = args
                    .iter()
                    .map(|arg| self.lower_expr(arg))
                    .collect::<Result<Vec<_>, _>>()?;
                lowered_args.push(left);
                Ok(hir::Expr::Call {
                    callee,
                    args: lowered_args,
                    span,
                })
            }
            other => Ok(hir::Expr::Call {
                callee: Box::new(self.lower_expr(other)?),
                args: vec![left],
                span,
            }),
        }
    }

    fn declare_pattern(&mut self, pattern: &ast::Pattern) -> Result<hir::Pattern, LowerError> {
        match pattern {
            ast::Pattern::Identifier(identifier) => Ok(hir::Pattern::Identifier(
                self.declare_binding(&identifier.name, identifier.span)?,
            )),
            ast::Pattern::Record { fields, span } => Ok(hir::Pattern::Record {
                fields: fields
                    .iter()
                    .map(|field| {
                        let binding = if field.pattern.is_none() {
                            Some(self.declare_binding(&field.name.name, field.name.span)?)
                        } else {
                            None
                        };
                        Ok(hir::RecordPatternField {
                            name: field.name.name.clone(),
                            binding,
                            pattern: field
                                .pattern
                                .as_ref()
                                .map(|pattern| self.declare_pattern(pattern))
                                .transpose()?
                                .map(Box::new),
                            span: field.span,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::Pattern::Array { items, span } => Ok(hir::Pattern::Array {
                items: items
                    .iter()
                    .map(|item| self.declare_pattern(item))
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::Pattern::Literal(literal) => {
                Ok(hir::Pattern::Literal(lower_literal_pattern(literal)))
            }
        }
    }

    fn lower_type_expr(&mut self, expr: &ast::TypeExpr) -> Result<hir::TypeExpr, LowerError> {
        match expr {
            ast::TypeExpr::Identifier(identifier) => Ok(hir::TypeExpr::Reference {
                reference: self.resolve_type_reference(&identifier.name, identifier.span)?,
                span: identifier.span,
            }),
            ast::TypeExpr::Literal(literal) => {
                Ok(hir::TypeExpr::Literal(lower_literal_type(literal)))
            }
            ast::TypeExpr::Record { fields, span } => Ok(hir::TypeExpr::Record {
                fields: fields
                    .iter()
                    .map(|field| {
                        Ok(hir::RecordTypeField {
                            name: field.name.name.clone(),
                            span: field.span,
                            value: self.lower_type_expr(&field.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::TypeExpr::Function {
                params,
                return_type,
                span,
            } => Ok(hir::TypeExpr::Function {
                params: params
                    .iter()
                    .map(|param| {
                        Ok(hir::FunctionTypeParam {
                            name: param.name.name.clone(),
                            span: param.span,
                            value: self.lower_type_expr(&param.value)?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?,
                return_type: Box::new(self.lower_type_expr(return_type)?),
                span: *span,
            }),
            ast::TypeExpr::Generic { name, args, span } => Ok(hir::TypeExpr::Apply {
                callee: self.resolve_type_reference(&name.name, name.span)?,
                args: args
                    .iter()
                    .map(|arg| self.lower_type_expr(arg))
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::TypeExpr::Array { element, span } => Ok(hir::TypeExpr::Array {
                element: Box::new(self.lower_type_expr(element)?),
                span: *span,
            }),
            ast::TypeExpr::Union { members, span } => Ok(hir::TypeExpr::Union {
                members: members
                    .iter()
                    .map(|member| self.lower_type_expr(member))
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::TypeExpr::Intersection { members, span } => Ok(hir::TypeExpr::Intersection {
                members: members
                    .iter()
                    .map(|member| self.lower_type_expr(member))
                    .collect::<Result<Vec<_>, _>>()?,
                span: *span,
            }),
            ast::TypeExpr::Grouped { inner, .. } => self.lower_type_expr(inner),
        }
    }

    fn declare_binding(&mut self, name: &str, span: Span) -> Result<hir::BindingName, LowerError> {
        let scope = self
            .value_scopes
            .last_mut()
            .expect("value scopes always contain a root scope");
        if scope.contains_key(name) {
            return Err(LowerError::DuplicateBinding {
                name: name.to_owned(),
                span,
            });
        }

        let binding = hir::BindingName {
            id: hir::BindingId(self.next_binding_id),
            name: name.to_owned(),
            span,
        };
        self.next_binding_id += 1;
        scope.insert(name.to_owned(), binding.clone());
        Ok(binding)
    }

    fn declare_type_alias(&mut self, name: &str, span: Span) -> Result<hir::TypeName, LowerError> {
        let scope = self
            .type_scopes
            .last_mut()
            .expect("type scopes always contain a root scope");
        if scope.contains_key(name) {
            return Err(LowerError::DuplicateType {
                name: name.to_owned(),
                span,
            });
        }

        let declared = hir::TypeName {
            id: hir::TypeId(self.next_type_id),
            name: name.to_owned(),
            span,
        };
        self.next_type_id += 1;
        scope.insert(
            name.to_owned(),
            ResolvedTypeName::AliasDeclaration(declared.clone()),
        );
        Ok(declared)
    }

    fn declare_type_param(&mut self, name: &str, span: Span) -> Result<hir::TypeParam, LowerError> {
        let scope = self
            .type_scopes
            .last_mut()
            .expect("type scopes always contain a root scope");
        if scope.contains_key(name) {
            return Err(LowerError::DuplicateType {
                name: name.to_owned(),
                span,
            });
        }

        let declared = hir::TypeParam {
            id: hir::TypeParamId(self.next_type_param_id),
            name: name.to_owned(),
            span,
        };
        self.next_type_param_id += 1;
        scope.insert(
            name.to_owned(),
            ResolvedTypeName::Reference(hir::TypeReference::TypeParam(declared.clone())),
        );
        Ok(declared)
    }

    fn resolve_identifier(&self, name: &str, span: Span) -> Result<hir::NameRef, LowerError> {
        self.value_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .cloned()
            .map(|binding| hir::NameRef {
                id: binding.id,
                name: binding.name,
                span,
            })
            .ok_or_else(|| LowerError::UnknownIdentifier {
                name: name.to_owned(),
                span,
            })
    }

    fn resolve_type_reference(
        &self,
        name: &str,
        span: Span,
    ) -> Result<hir::TypeReference, LowerError> {
        let resolved = self
            .type_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name))
            .ok_or_else(|| LowerError::UnknownType {
                name: name.to_owned(),
                span,
            })?;

        match resolved {
            ResolvedTypeName::Reference(reference) => Ok(reference.clone()),
            ResolvedTypeName::AliasDeclaration(alias) => {
                Ok(hir::TypeReference::Alias(hir::TypeName {
                    id: alias.id,
                    name: alias.name.clone(),
                    span,
                }))
            }
        }
    }

    fn install_builtin_types(&mut self) {
        let scope = self
            .type_scopes
            .last_mut()
            .expect("type scopes always contain a root scope");
        scope.extend(BTreeMap::from([
            (
                "Number".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinPrimitive(
                    hir::BuiltinPrimitive::Number,
                )),
            ),
            (
                "String".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinPrimitive(
                    hir::BuiltinPrimitive::String,
                )),
            ),
            (
                "Boolean".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinPrimitive(
                    hir::BuiltinPrimitive::Boolean,
                )),
            ),
            (
                "Null".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinPrimitive(
                    hir::BuiltinPrimitive::Null,
                )),
            ),
            (
                "Undefined".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinPrimitive(
                    hir::BuiltinPrimitive::Undefined,
                )),
            ),
            (
                "Never".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinPrimitive(
                    hir::BuiltinPrimitive::Never,
                )),
            ),
            (
                "Unknown".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinPrimitive(
                    hir::BuiltinPrimitive::Unknown,
                )),
            ),
            (
                "Sequence".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinGeneric(
                    hir::BuiltinGeneric::Sequence,
                )),
            ),
            (
                "Result".to_owned(),
                ResolvedTypeName::Reference(hir::TypeReference::BuiltinGeneric(
                    hir::BuiltinGeneric::Result,
                )),
            ),
        ]));
    }

    fn push_value_scope(&mut self) {
        self.value_scopes.push(BTreeMap::new());
    }

    fn pop_value_scope(&mut self) {
        let popped = self.value_scopes.pop();
        debug_assert!(popped.is_some());
    }

    fn push_type_scope(&mut self) {
        self.type_scopes.push(BTreeMap::new());
    }

    fn pop_type_scope(&mut self) {
        let popped = self.type_scopes.pop();
        debug_assert!(popped.is_some());
    }
}

fn lower_unary_operator(operator: ast::UnaryOperator) -> hir::UnaryOperator {
    match operator {
        ast::UnaryOperator::Not => hir::UnaryOperator::Not,
        ast::UnaryOperator::Negate => hir::UnaryOperator::Negate,
        ast::UnaryOperator::Positive => hir::UnaryOperator::Positive,
        ast::UnaryOperator::Defer => hir::UnaryOperator::Defer,
    }
}

fn lower_binary_operator(operator: ast::BinaryOperator) -> hir::BinaryOperator {
    match operator {
        ast::BinaryOperator::LogicalOr => hir::BinaryOperator::LogicalOr,
        ast::BinaryOperator::LogicalAnd => hir::BinaryOperator::LogicalAnd,
        ast::BinaryOperator::NullishCoalesce => hir::BinaryOperator::NullishCoalesce,
        ast::BinaryOperator::StrictEqual => hir::BinaryOperator::StrictEqual,
        ast::BinaryOperator::StrictNotEqual => hir::BinaryOperator::StrictNotEqual,
        ast::BinaryOperator::Less => hir::BinaryOperator::Less,
        ast::BinaryOperator::LessEqual => hir::BinaryOperator::LessEqual,
        ast::BinaryOperator::Greater => hir::BinaryOperator::Greater,
        ast::BinaryOperator::GreaterEqual => hir::BinaryOperator::GreaterEqual,
        ast::BinaryOperator::Add => hir::BinaryOperator::Add,
        ast::BinaryOperator::Subtract => hir::BinaryOperator::Subtract,
        ast::BinaryOperator::Multiply => hir::BinaryOperator::Multiply,
        ast::BinaryOperator::Divide => hir::BinaryOperator::Divide,
        ast::BinaryOperator::Modulo => hir::BinaryOperator::Modulo,
    }
}

fn lower_literal_pattern(pattern: &ast::LiteralPattern) -> hir::LiteralPattern {
    match pattern {
        ast::LiteralPattern::String { value, span } => hir::LiteralPattern::String {
            value: value.clone(),
            span: *span,
        },
        ast::LiteralPattern::Number { value, span } => hir::LiteralPattern::Number {
            value: *value,
            span: *span,
        },
        ast::LiteralPattern::Boolean { value, span } => hir::LiteralPattern::Boolean {
            value: *value,
            span: *span,
        },
        ast::LiteralPattern::Null { span } => hir::LiteralPattern::Null { span: *span },
        ast::LiteralPattern::Undefined { span } => hir::LiteralPattern::Undefined { span: *span },
    }
}

fn lower_literal_type(literal: &ast::LiteralType) -> hir::LiteralType {
    match literal {
        ast::LiteralType::String { value, span } => hir::LiteralType::String {
            value: value.clone(),
            span: *span,
        },
        ast::LiteralType::Number { value, span } => hir::LiteralType::Number {
            value: *value,
            span: *span,
        },
        ast::LiteralType::Boolean { value, span } => hir::LiteralType::Boolean {
            value: *value,
            span: *span,
        },
    }
}

fn lower_module_item_to_ir(item: &hir::ModuleItem) -> Option<ir::ModuleItem> {
    match item {
        hir::ModuleItem::Import(import) => Some(ir::ModuleItem::Import(ir::ImportDecl {
            clause: match &import.clause {
                hir::ImportClause::Default(binding) => {
                    ir::ImportClause::Default(binding.name.clone())
                }
                hir::ImportClause::Named(bindings) => ir::ImportClause::Named(
                    bindings
                        .iter()
                        .map(|binding| binding.name.clone())
                        .collect(),
                ),
            },
            source: import.source.clone(),
            source_span: import.source_span,
            span: import.span,
        })),
        hir::ModuleItem::Binding(binding) => {
            Some(ir::ModuleItem::Binding(lower_binding_to_ir(binding)))
        }
        hir::ModuleItem::Type(_) => None,
    }
}

fn lower_binding_to_ir(binding: &hir::BindingDecl) -> ir::BindingDecl {
    ir::BindingDecl {
        pattern: lower_pattern_to_ir(&binding.pattern),
        value: lower_expr_to_ir(&binding.value),
        is_exported: binding.is_exported,
        span: binding.span,
    }
}

fn collect_exported_names(pattern: &hir::Pattern) -> Option<Vec<String>> {
    match pattern {
        hir::Pattern::Identifier(binding) => Some(vec![binding.name.clone()]),
        hir::Pattern::Record { fields, .. } => Some(
            fields
                .iter()
                .flat_map(|field| {
                    if let Some(pattern) = &field.pattern {
                        collect_exported_names(pattern).unwrap_or_default()
                    } else {
                        vec![match &field.binding {
                            Some(binding) => binding.name.clone(),
                            None => field.name.clone(),
                        }]
                    }
                })
                .collect(),
        ),
        hir::Pattern::Array { items, .. } => Some(
            items
                .iter()
                .flat_map(|pattern| collect_exported_names(pattern).unwrap_or_default())
                .collect(),
        ),
        hir::Pattern::Literal(_) => None,
    }
}

fn lower_pattern_to_ir(pattern: &hir::Pattern) -> ir::Pattern {
    match pattern {
        hir::Pattern::Identifier(binding) => ir::Pattern::Identifier {
            name: binding.name.clone(),
            span: binding.span,
        },
        hir::Pattern::Record { fields, span } => ir::Pattern::Record {
            fields: fields
                .iter()
                .map(|field| ir::RecordPatternField {
                    name: field.name.clone(),
                    binding: field.binding.as_ref().map(|binding| binding.name.clone()),
                    pattern: field
                        .pattern
                        .as_ref()
                        .map(|pattern| Box::new(lower_pattern_to_ir(pattern))),
                    span: field.span,
                })
                .collect(),
            span: *span,
        },
        hir::Pattern::Array { items, span } => ir::Pattern::Array {
            items: items.iter().map(lower_pattern_to_ir).collect(),
            span: *span,
        },
        hir::Pattern::Literal(literal) => ir::Pattern::Literal(match literal {
            hir::LiteralPattern::String { value, span } => ir::LiteralPattern::String {
                value: value.clone(),
                span: *span,
            },
            hir::LiteralPattern::Number { value, span } => ir::LiteralPattern::Number {
                value: *value,
                span: *span,
            },
            hir::LiteralPattern::Boolean { value, span } => ir::LiteralPattern::Boolean {
                value: *value,
                span: *span,
            },
            hir::LiteralPattern::Null { span } => ir::LiteralPattern::Null { span: *span },
            hir::LiteralPattern::Undefined { span } => {
                ir::LiteralPattern::Undefined { span: *span }
            }
        }),
    }
}

fn lower_expr_to_ir(expr: &hir::Expr) -> ir::Expr {
    match expr {
        hir::Expr::StringLiteral { value, span } => ir::Expr::StringLiteral {
            value: value.clone(),
            span: *span,
        },
        hir::Expr::NumberLiteral { value, span } => ir::Expr::NumberLiteral {
            value: *value,
            span: *span,
        },
        hir::Expr::BooleanLiteral { value, span } => ir::Expr::BooleanLiteral {
            value: *value,
            span: *span,
        },
        hir::Expr::Null { span } => ir::Expr::Null { span: *span },
        hir::Expr::Undefined { span } => ir::Expr::Undefined { span: *span },
        hir::Expr::Identifier(identifier) => ir::Expr::Identifier {
            name: identifier.name.clone(),
            span: identifier.span,
        },
        hir::Expr::Record { fields, span } => ir::Expr::Record {
            fields: fields
                .iter()
                .map(|field| ir::RecordField {
                    name: field.name.clone(),
                    value: lower_expr_to_ir(&field.value),
                    span: field.span,
                })
                .collect(),
            span: *span,
        },
        hir::Expr::Array { items, span } => ir::Expr::Array {
            items: items.iter().map(lower_expr_to_ir).collect(),
            span: *span,
        },
        hir::Expr::Function {
            parameters,
            body,
            is_generator,
            span,
            ..
        } => ir::Expr::Function {
            parameters: parameters
                .iter()
                .map(|parameter| ir::Parameter {
                    pattern: lower_pattern_to_ir(&parameter.pattern),
                    span: parameter.span,
                })
                .collect(),
            body: Box::new(lower_expr_to_ir(body)),
            is_generator: *is_generator,
            span: *span,
        },
        hir::Expr::Block { items, span } => ir::Expr::Block {
            items: items
                .iter()
                .map(|item| match item {
                    hir::BlockItem::Binding(binding) => {
                        ir::BlockItem::Binding(lower_binding_to_ir(binding))
                    }
                    hir::BlockItem::Expr(expr) => ir::BlockItem::Expr(lower_expr_to_ir(expr)),
                })
                .collect(),
            span: *span,
        },
        hir::Expr::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => ir::Expr::If {
            condition: Box::new(lower_expr_to_ir(condition)),
            then_branch: Box::new(lower_expr_to_ir(then_branch)),
            else_branch: else_branch
                .as_ref()
                .map(|expr| Box::new(lower_expr_to_ir(expr))),
            span: *span,
        },
        hir::Expr::Match { value, arms, span } => ir::Expr::Match {
            value: Box::new(lower_expr_to_ir(value)),
            arms: arms
                .iter()
                .map(|arm| ir::MatchArm {
                    pattern: lower_pattern_to_ir(&arm.pattern),
                    body: lower_expr_to_ir(&arm.body),
                    span: arm.span,
                })
                .collect(),
            span: *span,
        },
        hir::Expr::Try {
            body,
            catch_pattern,
            catch_body,
            span,
        } => ir::Expr::Try {
            body: Box::new(lower_expr_to_ir(body)),
            catch_pattern: lower_pattern_to_ir(catch_pattern),
            catch_body: Box::new(lower_expr_to_ir(catch_body)),
            span: *span,
        },
        hir::Expr::Throw { value, span } => ir::Expr::Throw {
            value: Box::new(lower_expr_to_ir(value)),
            span: *span,
        },
        hir::Expr::Yield { value, span } => ir::Expr::Yield {
            value: Box::new(lower_expr_to_ir(value)),
            span: *span,
        },
        hir::Expr::Unary {
            operator,
            operand,
            span,
        } => ir::Expr::Unary {
            operator: match operator {
                hir::UnaryOperator::Not => ir::UnaryOperator::Not,
                hir::UnaryOperator::Negate => ir::UnaryOperator::Negate,
                hir::UnaryOperator::Positive => ir::UnaryOperator::Positive,
                hir::UnaryOperator::Defer => ir::UnaryOperator::Defer,
            },
            operand: Box::new(lower_expr_to_ir(operand)),
            span: *span,
        },
        hir::Expr::Binary {
            operator,
            left,
            right,
            span,
        } => ir::Expr::Binary {
            operator: match operator {
                hir::BinaryOperator::LogicalOr => ir::BinaryOperator::LogicalOr,
                hir::BinaryOperator::LogicalAnd => ir::BinaryOperator::LogicalAnd,
                hir::BinaryOperator::NullishCoalesce => ir::BinaryOperator::NullishCoalesce,
                hir::BinaryOperator::StrictEqual => ir::BinaryOperator::StrictEqual,
                hir::BinaryOperator::StrictNotEqual => ir::BinaryOperator::StrictNotEqual,
                hir::BinaryOperator::Less => ir::BinaryOperator::Less,
                hir::BinaryOperator::LessEqual => ir::BinaryOperator::LessEqual,
                hir::BinaryOperator::Greater => ir::BinaryOperator::Greater,
                hir::BinaryOperator::GreaterEqual => ir::BinaryOperator::GreaterEqual,
                hir::BinaryOperator::Add => ir::BinaryOperator::Add,
                hir::BinaryOperator::Subtract => ir::BinaryOperator::Subtract,
                hir::BinaryOperator::Multiply => ir::BinaryOperator::Multiply,
                hir::BinaryOperator::Divide => ir::BinaryOperator::Divide,
                hir::BinaryOperator::Modulo => ir::BinaryOperator::Modulo,
            },
            left: Box::new(lower_expr_to_ir(left)),
            right: Box::new(lower_expr_to_ir(right)),
            span: *span,
        },
        hir::Expr::Call { callee, args, span } => ir::Expr::Call {
            callee: Box::new(lower_expr_to_ir(callee)),
            args: args.iter().map(lower_expr_to_ir).collect(),
            span: *span,
        },
        hir::Expr::Member {
            object,
            property,
            span,
        } => ir::Expr::Member {
            object: Box::new(lower_expr_to_ir(object)),
            property: property.clone(),
            span: *span,
        },
        hir::Expr::Index {
            object,
            index,
            span,
        } => ir::Expr::Index {
            object: Box::new(lower_expr_to_ir(object)),
            index: Box::new(lower_expr_to_ir(index)),
            span: *span,
        },
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;
    use fscript_ir as ir;
    use fscript_lexer::lex;
    use fscript_parser::parse_module;

    use super::{LowerError, hir, lower_module, lower_to_ir};

    fn lower(text: &str) -> Result<hir::Module, LowerError> {
        let source = fscript_source::SourceFile::new(Utf8PathBuf::from("test.fs"), text.to_owned());
        let lexed = lex(&source);
        assert!(lexed.diagnostics.is_empty());
        let parsed = parse_module(&source, &lexed.tokens);
        assert!(parsed.diagnostics.is_empty());
        lower_module(&parsed.module)
    }

    #[test]
    fn lowers_pipe_expressions_into_calls() {
        let module = lower(
            "import Array from 'std:array'\n\
             numbers = [1, 2, 3]\n\
             result = numbers |> Array.length",
        )
        .expect("lowering should succeed");

        let hir::ModuleItem::Binding(binding) = &module.items[2] else {
            panic!("expected binding");
        };

        assert!(matches!(binding.value, hir::Expr::Call { .. }));
    }

    #[test]
    fn rejects_unknown_identifiers() {
        let error = lower("result = missing").expect_err("missing names should fail");

        assert!(matches!(error, LowerError::UnknownIdentifier { .. }));
    }

    #[test]
    fn lowers_hir_into_executable_ir() {
        let hir = lower(
            "import Result from 'std:result'\n\
             answer = try { Result.withDefault(0, throw Result.error(1)) } catch (error) { error }",
        )
        .expect("lowering should succeed");

        let module = lower_to_ir(&hir);

        let ir::ModuleItem::Binding(binding) = &module.items[1] else {
            panic!("expected lowered binding");
        };

        assert!(matches!(binding.value, ir::Expr::Try { .. }));
    }
}
