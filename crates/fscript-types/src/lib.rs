//! Type inference and checking for the current semantic slice.

use std::collections::{BTreeMap, BTreeSet};

use fscript_hir as hir;
use fscript_source::Span;
use thiserror::Error;

/// Typechecks a lowered module.
pub fn check_module(module: &hir::Module) -> Result<(), TypeError> {
    Checker::new(module)?.check_module(module)
}

/// A typechecking failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct TypeError {
    message: String,
    span: Span,
}

impl TypeError {
    /// Returns the error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Returns the source span for the diagnostic.
    #[must_use]
    pub const fn span(&self) -> Span {
        self.span
    }
}

struct Checker<'a> {
    aliases: BTreeMap<hir::TypeId, AliasDecl<'a>>,
    value_scopes: Vec<BTreeMap<hir::BindingId, Scheme>>,
    inference: InferenceContext,
    generator_yields: Vec<Type>,
}

#[derive(Clone, Copy)]
struct AliasDecl<'a> {
    params: &'a [hir::TypeParam],
    body: &'a hir::TypeExpr,
}

#[derive(Clone, Debug, PartialEq)]
struct Scheme {
    vars: Vec<String>,
    ty: Type,
}

impl Scheme {
    fn mono(ty: Type) -> Self {
        Self {
            vars: Vec::new(),
            ty,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Type {
    Primitive(PrimitiveType),
    Literal(LiteralValue),
    Array(Box<Type>),
    Sequence(Box<Type>),
    Result(Box<Type>, Box<Type>),
    Record(BTreeMap<String, Type>),
    Function(FunctionType),
    Union(Vec<Type>),
    Intersection(Vec<Type>),
    Module(StdModule),
    Var(TypeVarId),
    Generic(String),
    Never,
    Unknown,
}

#[derive(Clone, Debug, PartialEq)]
struct FunctionType {
    params: Vec<Type>,
    return_type: Box<Type>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PrimitiveType {
    Number,
    String,
    Boolean,
    Null,
    Undefined,
}

#[derive(Clone, Debug, PartialEq)]
enum LiteralValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StdModule {
    Array,
    Object,
    String,
    Number,
    Result,
    Json,
    Logger,
    Http,
    Filesystem,
    Task,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct TypeVarId(u32);

struct InferenceContext {
    next_var: u32,
    substitutions: BTreeMap<TypeVarId, Type>,
}

impl InferenceContext {
    fn new() -> Self {
        Self {
            next_var: 0,
            substitutions: BTreeMap::new(),
        }
    }

    fn fresh_var(&mut self) -> Type {
        let id = TypeVarId(self.next_var);
        self.next_var += 1;
        Type::Var(id)
    }
}

impl<'a> Checker<'a> {
    fn new(module: &'a hir::Module) -> Result<Self, TypeError> {
        let aliases = module
            .items
            .iter()
            .filter_map(|item| match item {
                hir::ModuleItem::Type(ty) => Some((
                    ty.name.id,
                    AliasDecl {
                        params: &ty.type_params,
                        body: &ty.value,
                    },
                )),
                _ => None,
            })
            .collect();

        Ok(Self {
            aliases,
            value_scopes: vec![BTreeMap::new()],
            inference: InferenceContext::new(),
            generator_yields: Vec::new(),
        })
    }

    fn check_module(&mut self, module: &hir::Module) -> Result<(), TypeError> {
        for item in &module.items {
            match item {
                hir::ModuleItem::Import(import) => self.check_import(import)?,
                hir::ModuleItem::Type(_) => {}
                hir::ModuleItem::Binding(binding) => self.check_binding(binding)?,
            }
        }

        Ok(())
    }

    fn check_import(&mut self, import: &hir::ImportDecl) -> Result<(), TypeError> {
        if let Some(module) = std_module_from_source(&import.source) {
            match &import.clause {
                hir::ImportClause::Default(binding) => {
                    self.bind(binding.id, Scheme::mono(Type::Module(module)));
                }
                hir::ImportClause::Named(bindings) => {
                    for binding in bindings {
                        let scheme = std_module_export_scheme(
                            module,
                            &binding.name,
                            binding.span,
                            &mut self.inference,
                        )?;
                        self.bind(binding.id, scheme);
                    }
                }
            }
        } else {
            match &import.clause {
                hir::ImportClause::Default(binding) => {
                    self.bind(binding.id, Scheme::mono(Type::Unknown));
                }
                hir::ImportClause::Named(bindings) => {
                    for binding in bindings {
                        self.bind(binding.id, Scheme::mono(Type::Unknown));
                    }
                }
            }
        }

        Ok(())
    }

    fn check_binding(&mut self, binding: &hir::BindingDecl) -> Result<(), TypeError> {
        let value_type = self.infer_expr(&binding.value)?;
        let value_type = self.apply(&value_type);
        let bindings = self.bind_pattern(&binding.pattern, &value_type, binding.span)?;
        for (binding_id, ty) in bindings {
            self.bind(binding_id, self.generalize(ty));
        }
        Ok(())
    }

    fn infer_expr(&mut self, expr: &hir::Expr) -> Result<Type, TypeError> {
        match expr {
            hir::Expr::StringLiteral { value, .. } => {
                Ok(Type::Literal(LiteralValue::String(value.clone())))
            }
            hir::Expr::NumberLiteral { value, .. } => {
                Ok(Type::Literal(LiteralValue::Number(*value)))
            }
            hir::Expr::BooleanLiteral { value, .. } => {
                Ok(Type::Literal(LiteralValue::Boolean(*value)))
            }
            hir::Expr::Null { .. } => Ok(Type::Primitive(PrimitiveType::Null)),
            hir::Expr::Undefined { .. } => Ok(Type::Primitive(PrimitiveType::Undefined)),
            hir::Expr::Identifier(identifier) => self.lookup(identifier.id, identifier.span),
            hir::Expr::Record { fields, .. } => {
                let mut record = BTreeMap::new();
                for field in fields {
                    record.insert(field.name.clone(), self.infer_expr(&field.value)?);
                }
                Ok(Type::Record(record))
            }
            hir::Expr::Array { items, .. } => {
                let element = if items.is_empty() {
                    Type::Unknown
                } else {
                    union_types(
                        items
                            .iter()
                            .map(|item| self.infer_expr(item))
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                };
                Ok(Type::Array(Box::new(element)))
            }
            hir::Expr::Function {
                parameters,
                return_type,
                body,
                is_generator,
                span,
            } => self.infer_function(parameters, return_type.as_ref(), body, *is_generator, *span),
            hir::Expr::Block { items, .. } => {
                self.push_scope();
                let mut last = Type::Primitive(PrimitiveType::Undefined);
                for item in items {
                    match item {
                        hir::BlockItem::Binding(binding) => self.check_binding(binding)?,
                        hir::BlockItem::Expr(expr) => {
                            last = self.infer_expr(expr)?;
                        }
                    }
                }
                self.pop_scope();
                Ok(last)
            }
            hir::Expr::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let condition_type = self.infer_expr(condition)?;
                self.expect_type(
                    condition_type,
                    Type::Primitive(PrimitiveType::Boolean),
                    *span,
                    "`if` conditions must evaluate to Boolean values",
                )?;
                let then_type = self.infer_expr(then_branch)?;
                let else_type = if let Some(else_branch) = else_branch {
                    self.infer_expr(else_branch)?
                } else {
                    Type::Primitive(PrimitiveType::Undefined)
                };
                Ok(merge_branch_types(vec![then_type, else_type]))
            }
            hir::Expr::Match { value, arms, span } => {
                let value_type = self.infer_expr(value)?;
                let mut arm_types = Vec::new();
                for arm in arms {
                    self.push_scope();
                    let bindings = self.bind_pattern(&arm.pattern, &value_type, arm.span)?;
                    for (binding_id, ty) in bindings {
                        self.bind(binding_id, Scheme::mono(ty));
                    }
                    arm_types.push(self.infer_expr(&arm.body)?);
                    self.pop_scope();
                }

                if arm_types.is_empty() {
                    return Err(TypeError {
                        message: "match expressions must contain at least one arm".to_owned(),
                        span: *span,
                    });
                }

                self.check_match_exhaustiveness(&value_type, arms, *span)?;
                Ok(merge_branch_types(arm_types))
            }
            hir::Expr::Try {
                body,
                catch_pattern,
                catch_body,
                ..
            } => {
                let body_type = self.infer_expr(body)?;
                self.push_scope();
                for (binding_id, ty) in bind_unknown_pattern(catch_pattern) {
                    self.bind(binding_id, Scheme::mono(ty));
                }
                let catch_type = self.infer_expr(catch_body)?;
                self.pop_scope();
                Ok(merge_branch_types(vec![body_type, catch_type]))
            }
            hir::Expr::Throw { value, .. } => {
                let _ = self.infer_expr(value)?;
                Ok(Type::Never)
            }
            hir::Expr::Yield { value, span } => {
                let yielded = self.infer_expr(value)?;
                let Some(current_yield) = self.generator_yields.last().cloned() else {
                    return Err(TypeError {
                        message: "`yield` is only valid inside generator functions".to_owned(),
                        span: *span,
                    });
                };
                self.unify(yielded.clone(), current_yield, *span)?;
                Ok(yielded)
            }
            hir::Expr::Unary {
                operator,
                operand,
                span,
            } => {
                let operand = self.infer_expr(operand)?;
                match operator {
                    hir::UnaryOperator::Not => {
                        self.expect_type(
                            operand,
                            Type::Primitive(PrimitiveType::Boolean),
                            *span,
                            "cannot apply `!` to a non-Boolean value",
                        )?;
                        Ok(Type::Primitive(PrimitiveType::Boolean))
                    }
                    hir::UnaryOperator::Negate | hir::UnaryOperator::Positive => {
                        self.expect_type(
                            operand,
                            Type::Primitive(PrimitiveType::Number),
                            *span,
                            "numeric unary operators require Number values",
                        )?;
                        Ok(Type::Primitive(PrimitiveType::Number))
                    }
                    hir::UnaryOperator::Defer => Ok(operand),
                }
            }
            hir::Expr::Binary {
                operator,
                left,
                right,
                span,
            } => self.infer_binary_expr(*operator, left, right, *span),
            hir::Expr::Call { callee, args, span } => self.infer_call(callee, args, *span),
            hir::Expr::Member {
                object,
                property,
                span,
            } => self.infer_member(object, property, *span),
            hir::Expr::Index {
                object,
                index,
                span,
            } => {
                let index_type = self.infer_expr(index)?;
                self.expect_type(
                    index_type,
                    Type::Primitive(PrimitiveType::Number),
                    *span,
                    "array indexes must be Number values",
                )?;
                let object_type = self.infer_expr(object)?;
                let object = self.apply(&object_type);
                match object {
                    Type::Array(element) | Type::Sequence(element) => Ok(*element),
                    other => Err(TypeError {
                        message: format!("cannot index into {}", self.describe_type(&other)),
                        span: *span,
                    }),
                }
            }
        }
    }

    fn infer_function(
        &mut self,
        parameters: &[hir::Parameter],
        return_type: Option<&hir::TypeExpr>,
        body: &hir::Expr,
        is_generator: bool,
        span: Span,
    ) -> Result<Type, TypeError> {
        self.push_scope();
        let mut parameter_types = Vec::new();
        for parameter in parameters {
            let Some(annotation) = &parameter.type_annotation else {
                return Err(TypeError {
                    message:
                        "function parameters must have type annotations in the current semantic slice"
                            .to_owned(),
                    span: parameter.span,
                });
            };
            let ty = self.resolve_type_expr(annotation)?;
            parameter_types.push(ty.clone());
            for (binding_id, binding_type) in
                self.bind_pattern(&parameter.pattern, &ty, parameter.span)?
            {
                self.bind(binding_id, Scheme::mono(binding_type));
            }
        }

        let declared_return = return_type
            .map(|annotation| self.resolve_type_expr(annotation))
            .transpose()?;

        let function_return = if is_generator {
            let yielded_type = self.inference.fresh_var();
            self.generator_yields.push(yielded_type.clone());
            let _ = self.infer_expr(body)?;
            let yielded = self
                .generator_yields
                .pop()
                .expect("generator yield tracker should be present");
            let yielded_type = self.apply(&yielded);
            let inferred = Type::Sequence(Box::new(default_never(yielded_type)));
            if let Some(expected) = declared_return {
                self.unify(inferred.clone(), expected.clone(), span)?;
                expected
            } else {
                inferred
            }
        } else {
            let body_type = self.infer_expr(body)?;
            if let Some(expected) = declared_return {
                self.unify(body_type.clone(), expected.clone(), span)?;
                expected
            } else {
                body_type
            }
        };

        self.pop_scope();

        Ok(Type::Function(FunctionType {
            params: parameter_types,
            return_type: Box::new(function_return),
        }))
    }

    fn infer_binary_expr(
        &mut self,
        operator: hir::BinaryOperator,
        left: &hir::Expr,
        right: &hir::Expr,
        span: Span,
    ) -> Result<Type, TypeError> {
        let left = self.infer_expr(left)?;
        let right = self.infer_expr(right)?;

        match operator {
            hir::BinaryOperator::Add => {
                let left = widen_literals(self.apply(&left));
                let right = widen_literals(self.apply(&right));
                match (&left, &right) {
                    (
                        Type::Primitive(PrimitiveType::Number),
                        Type::Primitive(PrimitiveType::Number),
                    ) => Ok(Type::Primitive(PrimitiveType::Number)),
                    (
                        Type::Primitive(PrimitiveType::String),
                        Type::Primitive(PrimitiveType::String),
                    )
                    | (
                        Type::Primitive(PrimitiveType::String),
                        Type::Primitive(PrimitiveType::Number),
                    )
                    | (
                        Type::Primitive(PrimitiveType::Number),
                        Type::Primitive(PrimitiveType::String),
                    ) => Ok(Type::Primitive(PrimitiveType::String)),
                    _ => Err(TypeError {
                        message: format!(
                            "cannot add {} and {}",
                            self.describe_type(&left),
                            self.describe_type(&right)
                        ),
                        span,
                    }),
                }
            }
            hir::BinaryOperator::Subtract
            | hir::BinaryOperator::Multiply
            | hir::BinaryOperator::Divide
            | hir::BinaryOperator::Modulo
            | hir::BinaryOperator::Less
            | hir::BinaryOperator::LessEqual
            | hir::BinaryOperator::Greater
            | hir::BinaryOperator::GreaterEqual => {
                self.expect_type(
                    left,
                    Type::Primitive(PrimitiveType::Number),
                    span,
                    "numeric operators require Number operands",
                )?;
                self.expect_type(
                    right,
                    Type::Primitive(PrimitiveType::Number),
                    span,
                    "numeric operators require Number operands",
                )?;
                Ok(match operator {
                    hir::BinaryOperator::Subtract
                    | hir::BinaryOperator::Multiply
                    | hir::BinaryOperator::Divide
                    | hir::BinaryOperator::Modulo => Type::Primitive(PrimitiveType::Number),
                    _ => Type::Primitive(PrimitiveType::Boolean),
                })
            }
            hir::BinaryOperator::LogicalOr | hir::BinaryOperator::LogicalAnd => {
                self.expect_type(
                    left,
                    Type::Primitive(PrimitiveType::Boolean),
                    span,
                    "logical operators require Boolean operands",
                )?;
                self.expect_type(
                    right,
                    Type::Primitive(PrimitiveType::Boolean),
                    span,
                    "logical operators require Boolean operands",
                )?;
                Ok(Type::Primitive(PrimitiveType::Boolean))
            }
            hir::BinaryOperator::NullishCoalesce => Ok(union_types(vec![left, right])),
            hir::BinaryOperator::StrictEqual | hir::BinaryOperator::StrictNotEqual => {
                self.ensure_comparable(&left, span)?;
                self.ensure_comparable(&right, span)?;
                self.unify(left, right, span)?;
                Ok(Type::Primitive(PrimitiveType::Boolean))
            }
        }
    }

    fn infer_call(
        &mut self,
        callee: &hir::Expr,
        args: &[hir::Expr],
        span: Span,
    ) -> Result<Type, TypeError> {
        let callee_type = self.infer_expr(callee)?;
        let callee = self.apply(&callee_type);
        if matches!(callee, Type::Unknown) {
            for argument in args {
                let _ = self.infer_expr(argument)?;
            }
            return Ok(Type::Unknown);
        }
        let Type::Function(function) = callee else {
            return Err(TypeError {
                message: format!("cannot call {}", self.describe_type(&callee)),
                span,
            });
        };

        if args.len() > function.params.len() {
            return Err(TypeError {
                message: format!(
                    "function expected {} arguments but received {}",
                    function.params.len(),
                    args.len()
                ),
                span,
            });
        }

        for (argument, parameter_type) in args.iter().zip(function.params.iter()) {
            let argument_type = self.infer_expr(argument)?;
            self.unify(argument_type, parameter_type.clone(), argument.span())?;
        }

        let remaining_params = function.params[args.len()..]
            .iter()
            .map(|param| self.apply(param))
            .collect::<Vec<_>>();
        if remaining_params.is_empty() {
            Ok(self.apply(&function.return_type))
        } else {
            Ok(Type::Function(FunctionType {
                params: remaining_params,
                return_type: Box::new(self.apply(&function.return_type)),
            }))
        }
    }

    fn infer_member(
        &mut self,
        object: &hir::Expr,
        property: &str,
        span: Span,
    ) -> Result<Type, TypeError> {
        let object_type = self.infer_expr(object)?;
        let object = self.apply(&object_type);
        match object {
            Type::Module(module) => {
                let scheme = std_module_export_scheme(module, property, span, &mut self.inference)?;
                Ok(self.instantiate(&scheme))
            }
            Type::Unknown => Ok(Type::Unknown),
            Type::Record(fields) => fields.get(property).cloned().ok_or_else(|| TypeError {
                message: format!("record values do not contain a `{property}` field"),
                span,
            }),
            Type::Union(members) => {
                let field_types = members
                    .iter()
                    .map(|member| match member {
                        Type::Record(fields) => fields.get(property).cloned(),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>()
                    .ok_or_else(|| TypeError {
                        message: format!(
                            "cannot read `{property}` from {}",
                            self.describe_type(&Type::Union(members.clone()))
                        ),
                        span,
                    })?;
                Ok(union_types(field_types))
            }
            other => Err(TypeError {
                message: format!(
                    "cannot read `{property}` from {}",
                    self.describe_type(&other)
                ),
                span,
            }),
        }
    }

    fn bind_pattern(
        &mut self,
        pattern: &hir::Pattern,
        expected: &Type,
        span: Span,
    ) -> Result<Vec<(hir::BindingId, Type)>, TypeError> {
        match pattern {
            hir::Pattern::Identifier(binding) => Ok(vec![(binding.id, expected.clone())]),
            hir::Pattern::Literal(pattern) => {
                self.ensure_literal_pattern_compatible(pattern, expected, span)?;
                Ok(Vec::new())
            }
            hir::Pattern::Array { items, .. } => {
                let expected = self.apply(expected);
                let element = match expected {
                    Type::Array(element) | Type::Sequence(element) => *element,
                    other => {
                        return Err(TypeError {
                            message: format!(
                                "array patterns require array or sequence values, found {}",
                                self.describe_type(&other)
                            ),
                            span,
                        });
                    }
                };

                let mut bindings = Vec::new();
                for item in items {
                    bindings.extend(self.bind_pattern(item, &element, span)?);
                }
                Ok(bindings)
            }
            hir::Pattern::Record { fields, .. } => self.bind_record_pattern(fields, expected, span),
        }
    }

    fn bind_record_pattern(
        &mut self,
        fields: &[hir::RecordPatternField],
        expected: &Type,
        span: Span,
    ) -> Result<Vec<(hir::BindingId, Type)>, TypeError> {
        let variants = expand_variants(&self.apply(expected));
        let narrowed = variants
            .into_iter()
            .filter(|variant| record_pattern_matches_variant(fields, variant))
            .collect::<Vec<_>>();

        if narrowed.is_empty() {
            return Err(TypeError {
                message: format!(
                    "record pattern is not compatible with {}",
                    self.describe_type(expected)
                ),
                span,
            });
        }

        let mut bindings = Vec::new();
        for field in fields {
            let field_types = narrowed
                .iter()
                .map(|variant| extract_record_field_type(variant, &field.name))
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| TypeError {
                    message: format!(
                        "field `{}` is not present in every matched variant",
                        field.name
                    ),
                    span: field.span,
                })?;
            let field_type = union_types(field_types);

            if let Some(pattern) = &field.pattern {
                bindings.extend(self.bind_pattern(pattern, &field_type, field.span)?);
            } else if let Some(binding) = &field.binding {
                bindings.push((binding.id, field_type));
            } else {
                return Err(TypeError {
                    message: format!("record pattern field `{}` is missing a binding", field.name),
                    span: field.span,
                });
            }
        }

        Ok(bindings)
    }

    fn check_match_exhaustiveness(
        &mut self,
        value_type: &Type,
        arms: &[hir::MatchArm],
        span: Span,
    ) -> Result<(), TypeError> {
        let Some(variants) = tagged_union_variants(&self.apply(value_type)) else {
            return Ok(());
        };

        let missing = variants
            .into_iter()
            .filter(|variant| {
                !arms
                    .iter()
                    .any(|arm| pattern_matches_type(&arm.pattern, &variant.ty))
            })
            .map(|variant| format!("`{}`", variant.tag))
            .collect::<Vec<_>>();

        if missing.is_empty() {
            return Ok(());
        }

        Err(TypeError {
            message: format!(
                "non-exhaustive match over tagged union; missing arms for {}",
                missing.join(", ")
            ),
            span,
        })
    }

    fn resolve_type_expr(&mut self, expr: &hir::TypeExpr) -> Result<Type, TypeError> {
        self.resolve_type_expr_with_subst(expr, &BTreeMap::new())
    }

    fn resolve_type_expr_with_subst(
        &mut self,
        expr: &hir::TypeExpr,
        subst: &BTreeMap<String, Type>,
    ) -> Result<Type, TypeError> {
        match expr {
            hir::TypeExpr::Reference { reference, span } => {
                self.resolve_type_reference(reference, *span, subst)
            }
            hir::TypeExpr::Literal(literal) => Ok(match literal {
                hir::LiteralType::String { value, .. } => {
                    Type::Literal(LiteralValue::String(value.clone()))
                }
                hir::LiteralType::Number { value, .. } => {
                    Type::Literal(LiteralValue::Number(*value))
                }
                hir::LiteralType::Boolean { value, .. } => {
                    Type::Literal(LiteralValue::Boolean(*value))
                }
            }),
            hir::TypeExpr::Record { fields, .. } => Ok(Type::Record(
                fields
                    .iter()
                    .map(|field| {
                        Ok((
                            field.name.clone(),
                            self.resolve_type_expr_with_subst(&field.value, subst)?,
                        ))
                    })
                    .collect::<Result<BTreeMap<_, _>, _>>()?,
            )),
            hir::TypeExpr::Function {
                params,
                return_type,
                ..
            } => Ok(Type::Function(FunctionType {
                params: params
                    .iter()
                    .map(|param| self.resolve_type_expr_with_subst(&param.value, subst))
                    .collect::<Result<Vec<_>, _>>()?,
                return_type: Box::new(self.resolve_type_expr_with_subst(return_type, subst)?),
            })),
            hir::TypeExpr::Apply { callee, args, span } => {
                let args = args
                    .iter()
                    .map(|arg| self.resolve_type_expr_with_subst(arg, subst))
                    .collect::<Result<Vec<_>, _>>()?;
                self.apply_type_reference(callee, args, *span, subst)
            }
            hir::TypeExpr::Array { element, .. } => Ok(Type::Array(Box::new(
                self.resolve_type_expr_with_subst(element, subst)?,
            ))),
            hir::TypeExpr::Union { members, .. } => Ok(union_types(
                members
                    .iter()
                    .map(|member| self.resolve_type_expr_with_subst(member, subst))
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            hir::TypeExpr::Intersection { members, .. } => {
                let resolved_members = members
                    .iter()
                    .map(|member| self.resolve_type_expr_with_subst(member, subst))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.simplify_intersection(resolved_members))
            }
        }
    }

    fn resolve_type_reference(
        &mut self,
        reference: &hir::TypeReference,
        span: Span,
        subst: &BTreeMap<String, Type>,
    ) -> Result<Type, TypeError> {
        match reference {
            hir::TypeReference::BuiltinPrimitive(primitive) => Ok(match primitive {
                hir::BuiltinPrimitive::Number => Type::Primitive(PrimitiveType::Number),
                hir::BuiltinPrimitive::String => Type::Primitive(PrimitiveType::String),
                hir::BuiltinPrimitive::Boolean => Type::Primitive(PrimitiveType::Boolean),
                hir::BuiltinPrimitive::Null => Type::Primitive(PrimitiveType::Null),
                hir::BuiltinPrimitive::Undefined => Type::Primitive(PrimitiveType::Undefined),
                hir::BuiltinPrimitive::Never => Type::Never,
                hir::BuiltinPrimitive::Unknown => Type::Unknown,
            }),
            hir::TypeReference::BuiltinGeneric(_) => Err(TypeError {
                message: "generic built-in types must include type arguments".to_owned(),
                span,
            }),
            hir::TypeReference::Alias(_alias) => {
                self.apply_type_reference(reference, Vec::new(), span, subst)
            }
            hir::TypeReference::TypeParam(param) => {
                subst.get(&param.name).cloned().ok_or_else(|| TypeError {
                    message: format!("unbound type parameter `{}`", param.name),
                    span,
                })
            }
        }
    }

    fn apply_type_reference(
        &mut self,
        reference: &hir::TypeReference,
        args: Vec<Type>,
        span: Span,
        outer_subst: &BTreeMap<String, Type>,
    ) -> Result<Type, TypeError> {
        match reference {
            hir::TypeReference::BuiltinPrimitive(_) | hir::TypeReference::TypeParam(_) => {
                Err(TypeError {
                    message: "type arguments can only be applied to generic types".to_owned(),
                    span,
                })
            }
            hir::TypeReference::BuiltinGeneric(generic) => match generic {
                hir::BuiltinGeneric::Sequence => match args.as_slice() {
                    [element] => Ok(Type::Sequence(Box::new(element.clone()))),
                    _ => Err(TypeError {
                        message: "Sequence expects exactly one type argument".to_owned(),
                        span,
                    }),
                },
                hir::BuiltinGeneric::Result => match args.as_slice() {
                    [ok, err] => Ok(Type::Result(Box::new(ok.clone()), Box::new(err.clone()))),
                    _ => Err(TypeError {
                        message: "Result expects exactly two type arguments".to_owned(),
                        span,
                    }),
                },
            },
            hir::TypeReference::Alias(alias) => {
                let declaration = self.aliases.get(&alias.id).ok_or_else(|| TypeError {
                    message: format!("unknown type `{}`", alias.name),
                    span,
                })?;
                if declaration.params.len() != args.len() {
                    return Err(TypeError {
                        message: format!(
                            "type `{}` expects {} type arguments but received {}",
                            alias.name,
                            declaration.params.len(),
                            args.len()
                        ),
                        span,
                    });
                }

                let mut subst = outer_subst.clone();
                for (param, arg) in declaration.params.iter().zip(args) {
                    subst.insert(param.name.clone(), arg);
                }

                self.resolve_type_expr_with_subst(declaration.body, &subst)
            }
        }
    }

    fn unify(&mut self, actual: Type, expected: Type, span: Span) -> Result<(), TypeError> {
        let actual = self.apply(&actual);
        let expected = self.apply(&expected);

        match (actual, expected) {
            (Type::Var(var), ty) | (ty, Type::Var(var)) => self.bind_var(var, ty, span),
            (Type::Primitive(left), Type::Primitive(right)) if left == right => Ok(()),
            (Type::Literal(left), Type::Literal(right)) if left == right => Ok(()),
            (Type::Literal(left), Type::Primitive(right))
                if literal_widens_to_primitive(&left, right) =>
            {
                Ok(())
            }
            (Type::Primitive(left), Type::Literal(right))
                if literal_widens_to_primitive(&right, left) =>
            {
                Ok(())
            }
            (Type::Array(left), Type::Array(right))
            | (Type::Sequence(left), Type::Sequence(right)) => self.unify(*left, *right, span),
            (Type::Result(left_ok, left_err), Type::Result(right_ok, right_err)) => {
                self.unify(*left_ok, *right_ok, span)?;
                self.unify(*left_err, *right_err, span)
            }
            (Type::Record(left), Type::Record(right)) => {
                if left.len() != right.len() || left.keys().ne(right.keys()) {
                    return Err(TypeError {
                        message: format!(
                            "expected {} but found {}",
                            self.describe_type(&Type::Record(right)),
                            self.describe_type(&Type::Record(left))
                        ),
                        span,
                    });
                }
                for (name, left_ty) in left {
                    self.unify(
                        left_ty,
                        right.get(&name).cloned().expect("keys were checked"),
                        span,
                    )?;
                }
                Ok(())
            }
            (Type::Function(left), Type::Function(right)) => {
                if left.params.len() != right.params.len() {
                    return Err(TypeError {
                        message: format!(
                            "expected {} but found {}",
                            self.describe_type(&Type::Function(right)),
                            self.describe_type(&Type::Function(left))
                        ),
                        span,
                    });
                }
                for (left_param, right_param) in left.params.into_iter().zip(right.params) {
                    self.unify(left_param, right_param, span)?;
                }
                self.unify(*left.return_type, *right.return_type, span)
            }
            (Type::Union(left), other) => {
                for member in left {
                    self.unify(member, other.clone(), span)?;
                }
                Ok(())
            }
            (other, Type::Union(right)) => {
                if right
                    .iter()
                    .any(|member| self.unify(other.clone(), member.clone(), span).is_ok())
                {
                    Ok(())
                } else {
                    Err(TypeError {
                        message: format!(
                            "expected {} but found {}",
                            self.describe_type(&Type::Union(right)),
                            self.describe_type(&other)
                        ),
                        span,
                    })
                }
            }
            (Type::Intersection(left), other) => {
                let left = self.simplify_intersection(left);
                self.unify(left, other, span)
            }
            (other, Type::Intersection(right)) => {
                let right = self.simplify_intersection(right);
                self.unify(other, right, span)
            }
            (Type::Never, _) | (_, Type::Unknown) | (Type::Unknown, _) => Ok(()),
            (left, right) if left == right => Ok(()),
            (left, right) => Err(TypeError {
                message: format!(
                    "expected {} but found {}",
                    self.describe_type(&right),
                    self.describe_type(&left)
                ),
                span,
            }),
        }
    }

    fn bind_var(&mut self, var: TypeVarId, ty: Type, span: Span) -> Result<(), TypeError> {
        if ty == Type::Var(var) {
            return Ok(());
        }
        if occurs(var, &ty, &self.inference.substitutions) {
            return Err(TypeError {
                message: "infinite type detected".to_owned(),
                span,
            });
        }
        self.inference.substitutions.insert(var, ty);
        Ok(())
    }

    fn apply(&self, ty: &Type) -> Type {
        match ty {
            Type::Var(var) => self
                .inference
                .substitutions
                .get(var)
                .map(|resolved| self.apply(resolved))
                .unwrap_or(Type::Var(*var)),
            Type::Array(element) => Type::Array(Box::new(self.apply(element))),
            Type::Sequence(element) => Type::Sequence(Box::new(self.apply(element))),
            Type::Result(ok, err) => {
                Type::Result(Box::new(self.apply(ok)), Box::new(self.apply(err)))
            }
            Type::Record(fields) => Type::Record(
                fields
                    .iter()
                    .map(|(name, value)| (name.clone(), self.apply(value)))
                    .collect(),
            ),
            Type::Function(function) => Type::Function(FunctionType {
                params: function
                    .params
                    .iter()
                    .map(|param| self.apply(param))
                    .collect(),
                return_type: Box::new(self.apply(&function.return_type)),
            }),
            Type::Union(members) => {
                union_types(members.iter().map(|member| self.apply(member)).collect())
            }
            Type::Intersection(members) => self
                .simplify_intersection(members.iter().map(|member| self.apply(member)).collect()),
            other => other.clone(),
        }
    }

    fn instantiate(&mut self, scheme: &Scheme) -> Type {
        let subst = scheme
            .vars
            .iter()
            .map(|name| (name.clone(), self.inference.fresh_var()))
            .collect::<BTreeMap<_, _>>();
        substitute_generics(&scheme.ty, &subst)
    }

    fn generalize(&self, ty: Type) -> Scheme {
        let mut vars = BTreeSet::new();
        collect_free_vars(&self.apply(&ty), &mut vars);
        let names = vars
            .iter()
            .map(|var| format!("T{}", var.0))
            .collect::<Vec<_>>();
        let subst = vars
            .iter()
            .map(|var| (*var, Type::Generic(format!("T{}", var.0))))
            .collect::<BTreeMap<_, _>>();
        Scheme {
            vars: names,
            ty: substitute_vars(&self.apply(&ty), &subst),
        }
    }

    fn expect_type(
        &mut self,
        actual: Type,
        expected: Type,
        span: Span,
        message: &str,
    ) -> Result<(), TypeError> {
        self.unify(actual, expected, span).map_err(|_| TypeError {
            message: message.to_owned(),
            span,
        })
    }

    fn ensure_comparable(&self, ty: &Type, span: Span) -> Result<(), TypeError> {
        match self.apply(ty) {
            Type::Function(_) | Type::Module(_) => Err(TypeError {
                message: "functions cannot be compared with `===` or `!==`".to_owned(),
                span,
            }),
            _ => Ok(()),
        }
    }

    fn simplify_intersection(&self, members: Vec<Type>) -> Type {
        let mut flattened = Vec::new();
        for member in members {
            match member {
                Type::Intersection(inner) => flattened.extend(inner),
                other => flattened.push(other),
            }
        }

        if flattened
            .iter()
            .all(|member| matches!(member, Type::Record(_)))
        {
            let mut merged = BTreeMap::new();
            for member in flattened {
                let Type::Record(fields) = member else {
                    unreachable!();
                };
                merged.extend(fields);
            }
            Type::Record(merged)
        } else {
            Type::Intersection(flattened)
        }
    }

    fn lookup(&mut self, binding_id: hir::BindingId, span: Span) -> Result<Type, TypeError> {
        let scheme = self
            .value_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&binding_id))
            .cloned()
            .ok_or_else(|| TypeError {
                message: "unknown identifier".to_owned(),
                span,
            })?;
        Ok(self.instantiate(&scheme))
    }

    fn bind(&mut self, binding_id: hir::BindingId, scheme: Scheme) {
        self.value_scopes
            .last_mut()
            .expect("value scopes always include a root scope")
            .insert(binding_id, scheme);
    }

    fn push_scope(&mut self) {
        self.value_scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        let popped = self.value_scopes.pop();
        debug_assert!(popped.is_some());
    }

    fn describe_type(&self, ty: &Type) -> String {
        describe_type(&self.apply(ty))
    }
}

fn std_module_from_source(source: &str) -> Option<StdModule> {
    match source {
        "std:array" => Some(StdModule::Array),
        "std:object" => Some(StdModule::Object),
        "std:string" => Some(StdModule::String),
        "std:number" => Some(StdModule::Number),
        "std:result" => Some(StdModule::Result),
        "std:json" => Some(StdModule::Json),
        "std:logger" => Some(StdModule::Logger),
        "std:http" => Some(StdModule::Http),
        "std:filesystem" => Some(StdModule::Filesystem),
        "std:task" => Some(StdModule::Task),
        _ => None,
    }
}

fn logger_handle_type() -> Type {
    Type::Record(BTreeMap::from([
        (
            "destination".to_owned(),
            Type::Union(vec![
                Type::Literal(LiteralValue::String("stdout".to_owned())),
                Type::Literal(LiteralValue::String("stderr".to_owned())),
            ]),
        ),
        (
            "level".to_owned(),
            Type::Union(vec![
                Type::Literal(LiteralValue::String("debug".to_owned())),
                Type::Literal(LiteralValue::String("info".to_owned())),
                Type::Literal(LiteralValue::String("warn".to_owned())),
                Type::Literal(LiteralValue::String("error".to_owned())),
            ]),
        ),
        (
            "name".to_owned(),
            Type::Union(vec![
                Type::Primitive(PrimitiveType::String),
                Type::Primitive(PrimitiveType::Null),
            ]),
        ),
    ]))
}

fn std_module_export_scheme(
    module: StdModule,
    name: &str,
    span: Span,
    inference: &mut InferenceContext,
) -> Result<Scheme, TypeError> {
    let scheme = match (module, name) {
        (StdModule::Array, "map") => Scheme {
            vars: vec!["T".to_owned(), "U".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![
                    Type::Function(FunctionType {
                        params: vec![Type::Generic("T".to_owned())],
                        return_type: Box::new(Type::Generic("U".to_owned())),
                    }),
                    Type::Array(Box::new(Type::Generic("T".to_owned()))),
                ],
                return_type: Box::new(Type::Array(Box::new(Type::Generic("U".to_owned())))),
            }),
        },
        (StdModule::Array, "filter") => Scheme {
            vars: vec!["T".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![
                    Type::Function(FunctionType {
                        params: vec![Type::Generic("T".to_owned())],
                        return_type: Box::new(Type::Primitive(PrimitiveType::Boolean)),
                    }),
                    Type::Array(Box::new(Type::Generic("T".to_owned()))),
                ],
                return_type: Box::new(Type::Array(Box::new(Type::Generic("T".to_owned())))),
            }),
        },
        (StdModule::Array, "length") => Scheme {
            vars: vec!["T".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Array(Box::new(Type::Generic("T".to_owned())))],
                return_type: Box::new(Type::Primitive(PrimitiveType::Number)),
            }),
        },
        (StdModule::Object, "spread") => Scheme {
            vars: vec!["L".to_owned(), "R".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Generic("L".to_owned()), Type::Generic("R".to_owned())],
                return_type: Box::new(Type::Intersection(vec![
                    Type::Generic("L".to_owned()),
                    Type::Generic("R".to_owned()),
                ])),
            }),
        },
        (StdModule::String, "trim")
        | (StdModule::String, "uppercase")
        | (StdModule::String, "lowercase") => Scheme::mono(Type::Function(FunctionType {
            params: vec![Type::Primitive(PrimitiveType::String)],
            return_type: Box::new(Type::Primitive(PrimitiveType::String)),
        })),
        (StdModule::String, "isDigits") => Scheme::mono(Type::Function(FunctionType {
            params: vec![Type::Primitive(PrimitiveType::String)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Boolean)),
        })),
        (StdModule::Number, "parse") => Scheme::mono(Type::Function(FunctionType {
            params: vec![Type::Primitive(PrimitiveType::String)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Number)),
        })),
        (StdModule::Result, "ok") => Scheme {
            vars: vec!["T".to_owned(), "E".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Generic("T".to_owned())],
                return_type: Box::new(Type::Result(
                    Box::new(Type::Generic("T".to_owned())),
                    Box::new(Type::Generic("E".to_owned())),
                )),
            }),
        },
        (StdModule::Result, "error") => Scheme {
            vars: vec!["T".to_owned(), "E".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Generic("E".to_owned())],
                return_type: Box::new(Type::Result(
                    Box::new(Type::Generic("T".to_owned())),
                    Box::new(Type::Generic("E".to_owned())),
                )),
            }),
        },
        (StdModule::Result, "isOk") | (StdModule::Result, "isError") => Scheme {
            vars: vec!["T".to_owned(), "E".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Result(
                    Box::new(Type::Generic("T".to_owned())),
                    Box::new(Type::Generic("E".to_owned())),
                )],
                return_type: Box::new(Type::Primitive(PrimitiveType::Boolean)),
            }),
        },
        (StdModule::Result, "withDefault") => Scheme {
            vars: vec!["T".to_owned(), "E".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![
                    Type::Generic("T".to_owned()),
                    Type::Result(
                        Box::new(Type::Generic("T".to_owned())),
                        Box::new(Type::Generic("E".to_owned())),
                    ),
                ],
                return_type: Box::new(Type::Generic("T".to_owned())),
            }),
        },
        (StdModule::Json, "parse") | (StdModule::Json, "jsonToObject") => {
            Scheme::mono(Type::Function(FunctionType {
                params: vec![Type::Primitive(PrimitiveType::String)],
                return_type: Box::new(Type::Unknown),
            }))
        }
        (StdModule::Json, "stringify") | (StdModule::Json, "jsonToString") => {
            Scheme::mono(Type::Function(FunctionType {
                params: vec![Type::Unknown],
                return_type: Box::new(Type::Primitive(PrimitiveType::String)),
            }))
        }
        (StdModule::Json, "jsonToPrettyString") => Scheme::mono(Type::Function(FunctionType {
            params: vec![Type::Unknown],
            return_type: Box::new(Type::Primitive(PrimitiveType::String)),
        })),
        (StdModule::Logger, "create") => Scheme::mono(Type::Function(FunctionType {
            params: vec![logger_handle_type()],
            return_type: Box::new(logger_handle_type()),
        })),
        (StdModule::Logger, "log")
        | (StdModule::Logger, "debug")
        | (StdModule::Logger, "info")
        | (StdModule::Logger, "warn")
        | (StdModule::Logger, "error") => Scheme::mono(Type::Function(FunctionType {
            params: vec![logger_handle_type(), Type::Primitive(PrimitiveType::String)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Undefined)),
        })),
        (StdModule::Logger, "prettyJson") => Scheme::mono(Type::Function(FunctionType {
            params: vec![logger_handle_type(), Type::Unknown],
            return_type: Box::new(Type::Primitive(PrimitiveType::Undefined)),
        })),
        (StdModule::Http, "serve") => Scheme::mono(Type::Function(FunctionType {
            params: vec![
                Type::Record(BTreeMap::from([
                    ("host".to_owned(), Type::Primitive(PrimitiveType::String)),
                    (
                        "maxRequests".to_owned(),
                        Type::Primitive(PrimitiveType::Number),
                    ),
                    ("port".to_owned(), Type::Primitive(PrimitiveType::Number)),
                ])),
                Type::Function(FunctionType {
                    params: vec![Type::Record(BTreeMap::from([
                        ("body".to_owned(), Type::Primitive(PrimitiveType::String)),
                        ("method".to_owned(), Type::Primitive(PrimitiveType::String)),
                        ("path".to_owned(), Type::Primitive(PrimitiveType::String)),
                    ]))],
                    return_type: Box::new(Type::Record(BTreeMap::from([
                        ("body".to_owned(), Type::Primitive(PrimitiveType::String)),
                        (
                            "contentType".to_owned(),
                            Type::Primitive(PrimitiveType::String),
                        ),
                        ("status".to_owned(), Type::Primitive(PrimitiveType::Number)),
                    ]))),
                }),
            ],
            return_type: Box::new(Type::Primitive(PrimitiveType::Undefined)),
        })),
        (StdModule::Filesystem, "readFile") => Scheme::mono(Type::Function(FunctionType {
            params: vec![Type::Primitive(PrimitiveType::String)],
            return_type: Box::new(Type::Primitive(PrimitiveType::String)),
        })),
        (StdModule::Filesystem, "writeFile") => Scheme::mono(Type::Function(FunctionType {
            params: vec![
                Type::Primitive(PrimitiveType::String),
                Type::Primitive(PrimitiveType::String),
            ],
            return_type: Box::new(Type::Primitive(PrimitiveType::Undefined)),
        })),
        (StdModule::Filesystem, "exists") => Scheme::mono(Type::Function(FunctionType {
            params: vec![Type::Primitive(PrimitiveType::String)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Boolean)),
        })),
        (StdModule::Filesystem, "deleteFile") => Scheme::mono(Type::Function(FunctionType {
            params: vec![Type::Primitive(PrimitiveType::String)],
            return_type: Box::new(Type::Primitive(PrimitiveType::Undefined)),
        })),
        (StdModule::Filesystem, "readDir") => Scheme::mono(Type::Function(FunctionType {
            params: vec![Type::Primitive(PrimitiveType::String)],
            return_type: Box::new(Type::Array(Box::new(Type::Primitive(
                PrimitiveType::String,
            )))),
        })),
        (StdModule::Task, "defer") => Scheme {
            vars: vec!["T".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Function(FunctionType {
                    params: Vec::new(),
                    return_type: Box::new(Type::Generic("T".to_owned())),
                })],
                return_type: Box::new(Type::Generic("T".to_owned())),
            }),
        },
        (StdModule::Task, "force") => Scheme {
            vars: vec!["T".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Generic("T".to_owned())],
                return_type: Box::new(Type::Generic("T".to_owned())),
            }),
        },
        (StdModule::Task, "all") => Scheme {
            vars: vec!["T".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Array(Box::new(union_types(vec![
                    Type::Generic("T".to_owned()),
                    Type::Function(FunctionType {
                        params: Vec::new(),
                        return_type: Box::new(Type::Generic("T".to_owned())),
                    }),
                ])))],
                return_type: Box::new(Type::Array(Box::new(Type::Generic("T".to_owned())))),
            }),
        },
        (StdModule::Task, "race") => Scheme {
            vars: vec!["T".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Array(Box::new(union_types(vec![
                    Type::Generic("T".to_owned()),
                    Type::Function(FunctionType {
                        params: Vec::new(),
                        return_type: Box::new(Type::Generic("T".to_owned())),
                    }),
                ])))],
                return_type: Box::new(Type::Generic("T".to_owned())),
            }),
        },
        (StdModule::Task, "spawn") => Scheme {
            vars: vec!["T".to_owned()],
            ty: Type::Function(FunctionType {
                params: vec![Type::Function(FunctionType {
                    params: Vec::new(),
                    return_type: Box::new(Type::Generic("T".to_owned())),
                })],
                return_type: Box::new(Type::Generic("T".to_owned())),
            }),
        },
        _ => {
            return Err(TypeError {
                message: format!("module does not export `{name}`"),
                span,
            });
        }
    };

    Ok(Scheme {
        ty: substitute_generics(
            &scheme.ty,
            &scheme
                .vars
                .iter()
                .map(|name| (name.clone(), inference.fresh_var()))
                .collect(),
        ),
        vars: Vec::new(),
    })
}

fn union_types(types: Vec<Type>) -> Type {
    let mut members = Vec::new();
    for ty in types {
        match ty {
            Type::Union(inner) => members.extend(inner),
            other => members.push(other),
        }
    }
    dedupe_types(members)
}

fn merge_branch_types(types: Vec<Type>) -> Type {
    let filtered = types
        .into_iter()
        .filter(|ty| *ty != Type::Never)
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        Type::Never
    } else {
        union_types(filtered)
    }
}

fn dedupe_types(types: Vec<Type>) -> Type {
    let mut unique = Vec::new();
    for ty in types {
        if unique.iter().all(|existing| *existing != ty) {
            unique.push(ty);
        }
    }

    match unique.as_slice() {
        [] => Type::Never,
        [single] => single.clone(),
        _ => Type::Union(unique),
    }
}

fn default_never(ty: Type) -> Type {
    match ty {
        Type::Var(_) => Type::Never,
        other => other,
    }
}

fn widen_literals(ty: Type) -> Type {
    match ty {
        Type::Literal(LiteralValue::String(_)) => Type::Primitive(PrimitiveType::String),
        Type::Literal(LiteralValue::Number(_)) => Type::Primitive(PrimitiveType::Number),
        Type::Literal(LiteralValue::Boolean(_)) => Type::Primitive(PrimitiveType::Boolean),
        other => other,
    }
}

fn literal_widens_to_primitive(literal: &LiteralValue, primitive: PrimitiveType) -> bool {
    matches!(
        (literal, primitive),
        (LiteralValue::String(_), PrimitiveType::String)
            | (LiteralValue::Number(_), PrimitiveType::Number)
            | (LiteralValue::Boolean(_), PrimitiveType::Boolean)
    )
}

fn occurs(var: TypeVarId, ty: &Type, substitutions: &BTreeMap<TypeVarId, Type>) -> bool {
    match ty {
        Type::Var(other) => {
            if *other == var {
                true
            } else if let Some(resolved) = substitutions.get(other) {
                occurs(var, resolved, substitutions)
            } else {
                false
            }
        }
        Type::Array(element) | Type::Sequence(element) => occurs(var, element, substitutions),
        Type::Result(ok, err) => occurs(var, ok, substitutions) || occurs(var, err, substitutions),
        Type::Record(fields) => fields
            .values()
            .any(|value| occurs(var, value, substitutions)),
        Type::Function(function) => {
            function
                .params
                .iter()
                .any(|param| occurs(var, param, substitutions))
                || occurs(var, &function.return_type, substitutions)
        }
        Type::Union(members) | Type::Intersection(members) => members
            .iter()
            .any(|member| occurs(var, member, substitutions)),
        Type::Primitive(_)
        | Type::Literal(_)
        | Type::Module(_)
        | Type::Generic(_)
        | Type::Never
        | Type::Unknown => false,
    }
}

fn collect_free_vars(ty: &Type, vars: &mut BTreeSet<TypeVarId>) {
    match ty {
        Type::Var(var) => {
            vars.insert(*var);
        }
        Type::Array(element) | Type::Sequence(element) => collect_free_vars(element, vars),
        Type::Result(ok, err) => {
            collect_free_vars(ok, vars);
            collect_free_vars(err, vars);
        }
        Type::Record(fields) => {
            for value in fields.values() {
                collect_free_vars(value, vars);
            }
        }
        Type::Function(function) => {
            for param in &function.params {
                collect_free_vars(param, vars);
            }
            collect_free_vars(&function.return_type, vars);
        }
        Type::Union(members) | Type::Intersection(members) => {
            for member in members {
                collect_free_vars(member, vars);
            }
        }
        Type::Primitive(_)
        | Type::Literal(_)
        | Type::Module(_)
        | Type::Generic(_)
        | Type::Never
        | Type::Unknown => {}
    }
}

fn substitute_vars(ty: &Type, subst: &BTreeMap<TypeVarId, Type>) -> Type {
    match ty {
        Type::Var(var) => subst.get(var).cloned().unwrap_or(Type::Var(*var)),
        Type::Array(element) => Type::Array(Box::new(substitute_vars(element, subst))),
        Type::Sequence(element) => Type::Sequence(Box::new(substitute_vars(element, subst))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute_vars(ok, subst)),
            Box::new(substitute_vars(err, subst)),
        ),
        Type::Record(fields) => Type::Record(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), substitute_vars(value, subst)))
                .collect(),
        ),
        Type::Function(function) => Type::Function(FunctionType {
            params: function
                .params
                .iter()
                .map(|param| substitute_vars(param, subst))
                .collect(),
            return_type: Box::new(substitute_vars(&function.return_type, subst)),
        }),
        Type::Union(members) => Type::Union(
            members
                .iter()
                .map(|member| substitute_vars(member, subst))
                .collect(),
        ),
        Type::Intersection(members) => Type::Intersection(
            members
                .iter()
                .map(|member| substitute_vars(member, subst))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn substitute_generics(ty: &Type, subst: &BTreeMap<String, Type>) -> Type {
    match ty {
        Type::Generic(name) => subst
            .get(name)
            .cloned()
            .unwrap_or(Type::Generic(name.clone())),
        Type::Array(element) => Type::Array(Box::new(substitute_generics(element, subst))),
        Type::Sequence(element) => Type::Sequence(Box::new(substitute_generics(element, subst))),
        Type::Result(ok, err) => Type::Result(
            Box::new(substitute_generics(ok, subst)),
            Box::new(substitute_generics(err, subst)),
        ),
        Type::Record(fields) => Type::Record(
            fields
                .iter()
                .map(|(name, value)| (name.clone(), substitute_generics(value, subst)))
                .collect(),
        ),
        Type::Function(function) => Type::Function(FunctionType {
            params: function
                .params
                .iter()
                .map(|param| substitute_generics(param, subst))
                .collect(),
            return_type: Box::new(substitute_generics(&function.return_type, subst)),
        }),
        Type::Union(members) => union_types(
            members
                .iter()
                .map(|member| substitute_generics(member, subst))
                .collect(),
        ),
        Type::Intersection(members) => Type::Intersection(
            members
                .iter()
                .map(|member| substitute_generics(member, subst))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn expand_variants(ty: &Type) -> Vec<Type> {
    match ty {
        Type::Union(members) => members.clone(),
        Type::Result(ok, err) => vec![
            Type::Record(BTreeMap::from([
                (
                    "tag".to_owned(),
                    Type::Literal(LiteralValue::String("ok".to_owned())),
                ),
                ("value".to_owned(), (*ok.clone())),
            ])),
            Type::Record(BTreeMap::from([
                (
                    "tag".to_owned(),
                    Type::Literal(LiteralValue::String("error".to_owned())),
                ),
                ("error".to_owned(), (*err.clone())),
            ])),
        ],
        other => vec![other.clone()],
    }
}

#[derive(Clone, Debug, PartialEq)]
struct TaggedUnionVariant {
    tag: String,
    ty: Type,
}

fn tagged_union_variants(ty: &Type) -> Option<Vec<TaggedUnionVariant>> {
    let variants = expand_variants(ty);
    if variants.len() < 2 {
        return None;
    }

    variants
        .into_iter()
        .map(|variant| match &variant {
            Type::Record(fields) => match fields.get("tag") {
                Some(Type::Literal(LiteralValue::String(tag))) => Some(TaggedUnionVariant {
                    tag: tag.clone(),
                    ty: variant.clone(),
                }),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn extract_record_field_type(ty: &Type, field: &str) -> Option<Type> {
    match ty {
        Type::Record(fields) => fields.get(field).cloned(),
        _ => None,
    }
}

fn pattern_matches_type(pattern: &hir::Pattern, ty: &Type) -> bool {
    match pattern {
        hir::Pattern::Identifier(_) => true,
        hir::Pattern::Literal(pattern) => literal_pattern_matches_type(pattern, ty),
        hir::Pattern::Record { fields, .. } => record_pattern_matches_variant(fields, ty),
        hir::Pattern::Array { .. } => matches!(ty, Type::Array(_) | Type::Sequence(_)),
    }
}

fn record_pattern_matches_variant(fields: &[hir::RecordPatternField], variant: &Type) -> bool {
    let Type::Record(record) = variant else {
        return false;
    };

    fields.iter().all(|field| {
        let Some(field_type) = record.get(&field.name) else {
            return false;
        };
        match field.pattern.as_deref() {
            None => true,
            Some(hir::Pattern::Literal(pattern)) => {
                literal_pattern_matches_type(pattern, field_type)
            }
            Some(hir::Pattern::Identifier(_)) => true,
            Some(hir::Pattern::Record { fields, .. }) => {
                record_pattern_matches_variant(fields, field_type)
            }
            Some(hir::Pattern::Array { .. }) => {
                matches!(field_type, Type::Array(_) | Type::Sequence(_))
            }
        }
    })
}

fn literal_pattern_matches_type(pattern: &hir::LiteralPattern, ty: &Type) -> bool {
    match pattern {
        hir::LiteralPattern::String { value, .. } => match ty {
            Type::Literal(LiteralValue::String(actual)) => actual == value,
            Type::Primitive(PrimitiveType::String) => true,
            _ => false,
        },
        hir::LiteralPattern::Number { value, .. } => match ty {
            Type::Literal(LiteralValue::Number(actual)) => actual == value,
            Type::Primitive(PrimitiveType::Number) => true,
            _ => false,
        },
        hir::LiteralPattern::Boolean { value, .. } => match ty {
            Type::Literal(LiteralValue::Boolean(actual)) => actual == value,
            Type::Primitive(PrimitiveType::Boolean) => true,
            _ => false,
        },
        hir::LiteralPattern::Null { .. } => matches!(ty, Type::Primitive(PrimitiveType::Null)),
        hir::LiteralPattern::Undefined { .. } => {
            matches!(ty, Type::Primitive(PrimitiveType::Undefined))
        }
    }
}

fn bind_unknown_pattern(pattern: &hir::Pattern) -> Vec<(hir::BindingId, Type)> {
    match pattern {
        hir::Pattern::Identifier(binding) => vec![(binding.id, Type::Unknown)],
        hir::Pattern::Literal(_) => Vec::new(),
        hir::Pattern::Array { items, .. } => items.iter().flat_map(bind_unknown_pattern).collect(),
        hir::Pattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| {
                let mut bindings = field
                    .binding
                    .iter()
                    .map(|binding| (binding.id, Type::Unknown))
                    .collect::<Vec<_>>();
                if let Some(pattern) = field.pattern.as_deref() {
                    bindings.extend(bind_unknown_pattern(pattern));
                }
                bindings
            })
            .collect(),
    }
}

fn describe_type(ty: &Type) -> String {
    match ty {
        Type::Primitive(PrimitiveType::Number) => "Number".to_owned(),
        Type::Primitive(PrimitiveType::String) => "String".to_owned(),
        Type::Primitive(PrimitiveType::Boolean) => "Boolean".to_owned(),
        Type::Primitive(PrimitiveType::Null) => "Null".to_owned(),
        Type::Primitive(PrimitiveType::Undefined) => "Undefined".to_owned(),
        Type::Literal(LiteralValue::String(value)) => format!("{value:?}"),
        Type::Literal(LiteralValue::Number(value)) => value.to_string(),
        Type::Literal(LiteralValue::Boolean(value)) => value.to_string(),
        Type::Array(element) => format!("{}[]", describe_type(element)),
        Type::Sequence(element) => format!("Sequence<{}>", describe_type(element)),
        Type::Result(ok, err) => format!("Result<{}, {}>", describe_type(ok), describe_type(err)),
        Type::Record(fields) => {
            let parts = fields
                .iter()
                .map(|(name, value)| format!("{name}: {}", describe_type(value)))
                .collect::<Vec<_>>();
            format!("{{ {} }}", parts.join(", "))
        }
        Type::Function(function) => {
            let params = function
                .params
                .iter()
                .map(describe_type)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({params}): {}", describe_type(&function.return_type))
        }
        Type::Union(members) => members
            .iter()
            .map(describe_type)
            .collect::<Vec<_>>()
            .join(" | "),
        Type::Intersection(members) => members
            .iter()
            .map(describe_type)
            .collect::<Vec<_>>()
            .join(" & "),
        Type::Module(_) => "module".to_owned(),
        Type::Var(var) => format!("T{}", var.0),
        Type::Generic(name) => name.clone(),
        Type::Never => "Never".to_owned(),
        Type::Unknown => "Unknown".to_owned(),
    }
}

impl Checker<'_> {
    fn ensure_literal_pattern_compatible(
        &self,
        pattern: &hir::LiteralPattern,
        expected: &Type,
        span: Span,
    ) -> Result<(), TypeError> {
        if literal_pattern_matches_type(pattern, &self.apply(expected)) {
            Ok(())
        } else {
            Err(TypeError {
                message: format!(
                    "pattern is not compatible with {}",
                    self.describe_type(expected)
                ),
                span,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::check_module;
    use fscript_hir as hir;
    use fscript_source::Span;

    fn span() -> Span {
        Span::new(0, 1)
    }

    fn binding_name(id: u32, name: &str) -> hir::BindingName {
        hir::BindingName {
            id: hir::BindingId(id),
            name: name.to_owned(),
            span: span(),
        }
    }

    fn name_ref(id: u32, name: &str) -> hir::NameRef {
        hir::NameRef {
            id: hir::BindingId(id),
            name: name.to_owned(),
            span: span(),
        }
    }

    fn type_name(id: u32, name: &str) -> hir::TypeName {
        hir::TypeName {
            id: hir::TypeId(id),
            name: name.to_owned(),
            span: span(),
        }
    }

    fn type_param(id: u32, name: &str) -> hir::TypeParam {
        hir::TypeParam {
            id: hir::TypeParamId(id),
            name: name.to_owned(),
            span: span(),
        }
    }

    fn identifier_pattern(id: u32, name: &str) -> hir::Pattern {
        hir::Pattern::Identifier(binding_name(id, name))
    }

    fn identifier(id: u32, name: &str) -> hir::Expr {
        hir::Expr::Identifier(name_ref(id, name))
    }

    fn number(value: f64) -> hir::Expr {
        hir::Expr::NumberLiteral {
            value,
            span: span(),
        }
    }

    fn boolean(value: bool) -> hir::Expr {
        hir::Expr::BooleanLiteral {
            value,
            span: span(),
        }
    }

    fn string(value: &str) -> hir::Expr {
        hir::Expr::StringLiteral {
            value: value.to_owned(),
            span: span(),
        }
    }

    fn primitive_type(primitive: hir::BuiltinPrimitive) -> hir::TypeExpr {
        hir::TypeExpr::Reference {
            reference: hir::TypeReference::BuiltinPrimitive(primitive),
            span: span(),
        }
    }

    fn sequence_type(element: hir::TypeExpr) -> hir::TypeExpr {
        hir::TypeExpr::Apply {
            callee: hir::TypeReference::BuiltinGeneric(hir::BuiltinGeneric::Sequence),
            args: vec![element],
            span: span(),
        }
    }

    fn result_type(ok: hir::TypeExpr, err: hir::TypeExpr) -> hir::TypeExpr {
        hir::TypeExpr::Apply {
            callee: hir::TypeReference::BuiltinGeneric(hir::BuiltinGeneric::Result),
            args: vec![ok, err],
            span: span(),
        }
    }

    fn record_type(fields: Vec<(&str, hir::TypeExpr)>) -> hir::TypeExpr {
        hir::TypeExpr::Record {
            fields: fields
                .into_iter()
                .map(|(name, value)| hir::RecordTypeField {
                    name: name.to_owned(),
                    span: span(),
                    value,
                })
                .collect(),
            span: span(),
        }
    }

    fn import_default(id: u32, local: &str, source: &str) -> hir::ModuleItem {
        hir::ModuleItem::Import(hir::ImportDecl {
            clause: hir::ImportClause::Default(binding_name(id, local)),
            source: source.to_owned(),
            source_span: span(),
            span: span(),
        })
    }

    fn import_named(bindings: &[(u32, &str)], source: &str) -> hir::ModuleItem {
        hir::ModuleItem::Import(hir::ImportDecl {
            clause: hir::ImportClause::Named(
                bindings
                    .iter()
                    .map(|(id, name)| binding_name(*id, name))
                    .collect(),
            ),
            source: source.to_owned(),
            source_span: span(),
            span: span(),
        })
    }

    fn binding(id: u32, name: &str, value: hir::Expr) -> hir::ModuleItem {
        hir::ModuleItem::Binding(hir::BindingDecl {
            pattern: identifier_pattern(id, name),
            value,
            is_exported: false,
            span: span(),
        })
    }

    fn parameter(id: u32, name: &str, type_annotation: Option<hir::TypeExpr>) -> hir::Parameter {
        hir::Parameter {
            pattern: identifier_pattern(id, name),
            type_annotation,
            span: span(),
        }
    }

    fn function(
        parameters: Vec<hir::Parameter>,
        return_type: Option<hir::TypeExpr>,
        body: hir::Expr,
        is_generator: bool,
    ) -> hir::Expr {
        hir::Expr::Function {
            parameters,
            return_type,
            body: Box::new(body),
            is_generator,
            span: span(),
        }
    }

    fn module(items: Vec<hir::ModuleItem>) -> hir::Module {
        hir::Module { items }
    }

    #[test]
    fn accepts_std_named_imports_and_unknown_user_imports() {
        let module = module(vec![
            import_named(&[(0, "trim")], "std:string"),
            import_default(1, "UserModule", "./user.fs"),
            binding(
                2,
                "trimmed",
                hir::Expr::Call {
                    callee: Box::new(identifier(0, "trim")),
                    args: vec![string("  Ada  ")],
                    span: span(),
                },
            ),
            binding(
                3,
                "unknown_result",
                hir::Expr::Call {
                    callee: Box::new(identifier(1, "UserModule")),
                    args: vec![number(1.0)],
                    span: span(),
                },
            ),
        ]);

        check_module(&module).expect("std named imports and unknown user imports should typecheck");
    }

    #[test]
    fn rejects_unknown_std_named_import_exports() {
        let module = module(vec![import_named(&[(0, "missing")], "std:string")]);

        let error = check_module(&module).expect_err("unknown std exports should fail");

        assert!(error.message().contains("does not export `missing`"));
    }

    #[test]
    fn rejects_functions_without_parameter_annotations() {
        let module = module(vec![binding(
            0,
            "identity",
            function(
                vec![parameter(1, "value", None)],
                None,
                identifier(1, "value"),
                false,
            ),
        )]);

        let error = check_module(&module).expect_err("missing parameter annotations should fail");

        assert!(
            error
                .message()
                .contains("function parameters must have type annotations")
        );
    }

    #[test]
    fn rejects_yield_outside_generator_functions() {
        let module = module(vec![binding(
            0,
            "invalid",
            hir::Expr::Yield {
                value: Box::new(number(1.0)),
                span: span(),
            },
        )]);

        let error = check_module(&module).expect_err("top-level yield should fail");

        assert!(
            error
                .message()
                .contains("`yield` is only valid inside generator")
        );
    }

    #[test]
    fn accepts_generator_functions_with_matching_sequence_annotations() {
        let module = module(vec![binding(
            0,
            "counter",
            function(
                vec![],
                Some(sequence_type(primitive_type(hir::BuiltinPrimitive::Number))),
                hir::Expr::Yield {
                    value: Box::new(number(1.0)),
                    span: span(),
                },
                true,
            ),
        )]);

        check_module(&module).expect("matching generator annotations should typecheck");
    }

    #[test]
    fn rejects_generator_functions_with_mismatched_sequence_annotations() {
        let module = module(vec![binding(
            0,
            "counter",
            function(
                vec![],
                Some(sequence_type(primitive_type(hir::BuiltinPrimitive::String))),
                hir::Expr::Yield {
                    value: Box::new(number(1.0)),
                    span: span(),
                },
                true,
            ),
        )]);

        let error =
            check_module(&module).expect_err("mismatched generator annotations should fail");

        assert!(error.message().contains("expected"));
        assert!(error.message().contains("found"));
    }

    #[test]
    fn rejects_calling_non_function_values() {
        let module = module(vec![binding(
            0,
            "result",
            hir::Expr::Call {
                callee: Box::new(number(42.0)),
                args: vec![number(1.0)],
                span: span(),
            },
        )]);

        let error = check_module(&module).expect_err("calling numbers should fail");

        assert!(error.message().contains("cannot call"));
    }

    #[test]
    fn rejects_calls_with_too_many_arguments() {
        let module = module(vec![
            binding(
                0,
                "add_one",
                function(
                    vec![parameter(
                        1,
                        "value",
                        Some(primitive_type(hir::BuiltinPrimitive::Number)),
                    )],
                    Some(primitive_type(hir::BuiltinPrimitive::Number)),
                    hir::Expr::Binary {
                        operator: hir::BinaryOperator::Add,
                        left: Box::new(identifier(1, "value")),
                        right: Box::new(number(1.0)),
                        span: span(),
                    },
                    false,
                ),
            ),
            binding(
                2,
                "too_many",
                hir::Expr::Call {
                    callee: Box::new(identifier(0, "add_one")),
                    args: vec![number(1.0), number(2.0)],
                    span: span(),
                },
            ),
        ]);

        let error = check_module(&module).expect_err("extra call arguments should fail");

        assert!(
            error
                .message()
                .contains("expected 1 arguments but received 2")
        );
    }

    #[test]
    fn rejects_missing_record_fields_and_invalid_union_members() {
        let module = module(vec![
            binding(
                0,
                "record",
                hir::Expr::Record {
                    fields: vec![hir::RecordField {
                        name: "name".to_owned(),
                        span: span(),
                        value: string("Ada"),
                    }],
                    span: span(),
                },
            ),
            binding(
                1,
                "missing_field",
                hir::Expr::Member {
                    object: Box::new(identifier(0, "record")),
                    property: "age".to_owned(),
                    span: span(),
                },
            ),
        ]);

        let error = check_module(&module).expect_err("missing record fields should fail");

        assert!(error.message().contains("do not contain a `age` field"));
    }

    #[test]
    fn rejects_member_access_on_unions_without_shared_fields() {
        let user_type = hir::TypeDecl {
            name: type_name(0, "User"),
            type_params: Vec::new(),
            value: hir::TypeExpr::Union {
                members: vec![
                    record_type(vec![(
                        "tag",
                        hir::TypeExpr::Literal(hir::LiteralType::String {
                            value: "guest".to_owned(),
                            span: span(),
                        }),
                    )]),
                    record_type(vec![
                        (
                            "tag",
                            hir::TypeExpr::Literal(hir::LiteralType::String {
                                value: "member".to_owned(),
                                span: span(),
                            }),
                        ),
                        ("name", primitive_type(hir::BuiltinPrimitive::String)),
                    ]),
                ],
                span: span(),
            },
            is_exported: false,
            span: span(),
        };
        let module = module(vec![
            hir::ModuleItem::Type(user_type),
            binding(
                1,
                "user",
                hir::Expr::Record {
                    fields: vec![hir::RecordField {
                        name: "tag".to_owned(),
                        span: span(),
                        value: string("guest"),
                    }],
                    span: span(),
                },
            ),
            binding(
                2,
                "name",
                hir::Expr::Member {
                    object: Box::new(identifier(1, "user")),
                    property: "name".to_owned(),
                    span: span(),
                },
            ),
        ]);

        let error =
            check_module(&module).expect_err("union members without a shared field should fail");

        assert!(
            error.message().contains("do not contain a `name` field")
                || error.message().contains("cannot read `name`")
        );
    }

    #[test]
    fn rejects_invalid_index_targets_and_index_types() {
        let bad_target = module(vec![binding(
            0,
            "value",
            hir::Expr::Index {
                object: Box::new(number(1.0)),
                index: Box::new(number(0.0)),
                span: span(),
            },
        )]);
        let target_error = check_module(&bad_target).expect_err("indexing numbers should fail");
        assert!(target_error.message().contains("cannot index into"));

        let bad_index = module(vec![binding(
            0,
            "value",
            hir::Expr::Index {
                object: Box::new(hir::Expr::Array {
                    items: vec![number(1.0)],
                    span: span(),
                }),
                index: Box::new(boolean(true)),
                span: span(),
            },
        )]);
        let index_error = check_module(&bad_index).expect_err("boolean indexes should fail");
        assert!(
            index_error
                .message()
                .contains("array indexes must be Number values")
        );
    }

    #[test]
    fn rejects_invalid_unary_operands() {
        let not_module = module(vec![binding(
            0,
            "value",
            hir::Expr::Unary {
                operator: hir::UnaryOperator::Not,
                operand: Box::new(number(1.0)),
                span: span(),
            },
        )]);
        let not_error = check_module(&not_module).expect_err("`!` on numbers should fail");
        assert!(
            not_error
                .message()
                .contains("cannot apply `!` to a non-Boolean value")
        );

        let plus_module = module(vec![binding(
            0,
            "value",
            hir::Expr::Unary {
                operator: hir::UnaryOperator::Positive,
                operand: Box::new(string("Ada")),
                span: span(),
            },
        )]);
        let plus_error = check_module(&plus_module).expect_err("unary plus on strings should fail");
        assert!(
            plus_error
                .message()
                .contains("numeric unary operators require Number values")
        );
    }

    #[test]
    fn rejects_comparing_functions_with_strict_equality() {
        let module = module(vec![
            binding(
                0,
                "left",
                function(
                    vec![parameter(
                        1,
                        "value",
                        Some(primitive_type(hir::BuiltinPrimitive::Number)),
                    )],
                    Some(primitive_type(hir::BuiltinPrimitive::Number)),
                    identifier(1, "value"),
                    false,
                ),
            ),
            binding(
                2,
                "right",
                function(
                    vec![parameter(
                        3,
                        "value",
                        Some(primitive_type(hir::BuiltinPrimitive::Number)),
                    )],
                    Some(primitive_type(hir::BuiltinPrimitive::Number)),
                    identifier(3, "value"),
                    false,
                ),
            ),
            binding(
                4,
                "equal",
                hir::Expr::Binary {
                    operator: hir::BinaryOperator::StrictEqual,
                    left: Box::new(identifier(0, "left")),
                    right: Box::new(identifier(2, "right")),
                    span: span(),
                },
            ),
        ]);

        let error = check_module(&module).expect_err("function comparisons should fail");

        assert!(error.message().contains("functions cannot be compared"));
    }

    #[test]
    fn rejects_non_exhaustive_matches_and_empty_matches() {
        let non_exhaustive_module = module(vec![
            binding(
                0,
                "result",
                hir::Expr::Call {
                    callee: Box::new(hir::Expr::Member {
                        object: Box::new(identifier(10, "Result")),
                        property: "ok".to_owned(),
                        span: span(),
                    }),
                    args: vec![number(1.0)],
                    span: span(),
                },
            ),
            binding(
                1,
                "matched",
                hir::Expr::Match {
                    value: Box::new(identifier(0, "result")),
                    arms: vec![hir::MatchArm {
                        pattern: hir::Pattern::Record {
                            fields: vec![
                                hir::RecordPatternField {
                                    name: "tag".to_owned(),
                                    binding: None,
                                    pattern: Some(Box::new(hir::Pattern::Literal(
                                        hir::LiteralPattern::String {
                                            value: "ok".to_owned(),
                                            span: span(),
                                        },
                                    ))),
                                    span: span(),
                                },
                                hir::RecordPatternField {
                                    name: "value".to_owned(),
                                    binding: Some(binding_name(2, "value")),
                                    pattern: None,
                                    span: span(),
                                },
                            ],
                            span: span(),
                        },
                        body: identifier(2, "value"),
                        span: span(),
                    }],
                    span: span(),
                },
            ),
        ]);
        let result_imported = hir::Module {
            items: vec![
                import_default(10, "Result", "std:result"),
                non_exhaustive_module.items[0].clone(),
                non_exhaustive_module.items[1].clone(),
            ],
        };

        let non_exhaustive =
            check_module(&result_imported).expect_err("missing error arm should fail");
        assert!(non_exhaustive.message().contains("non-exhaustive match"));

        let empty_match = module(vec![binding(
            0,
            "matched",
            hir::Expr::Match {
                value: Box::new(number(1.0)),
                arms: Vec::new(),
                span: span(),
            },
        )]);

        let empty_error = check_module(&empty_match).expect_err("empty matches should fail");
        assert!(
            empty_error
                .message()
                .contains("match expressions must contain at least one arm")
        );
    }

    #[test]
    fn rejects_record_patterns_without_bindings_and_incompatible_patterns() {
        let missing_binding = module(vec![binding(
            0,
            "destructured",
            hir::Expr::Block {
                items: vec![
                    hir::BlockItem::Binding(hir::BindingDecl {
                        pattern: hir::Pattern::Record {
                            fields: vec![hir::RecordPatternField {
                                name: "name".to_owned(),
                                binding: None,
                                pattern: None,
                                span: span(),
                            }],
                            span: span(),
                        },
                        value: hir::Expr::Record {
                            fields: vec![hir::RecordField {
                                name: "name".to_owned(),
                                span: span(),
                                value: string("Ada"),
                            }],
                            span: span(),
                        },
                        is_exported: false,
                        span: span(),
                    }),
                    hir::BlockItem::Expr(number(1.0)),
                ],
                span: span(),
            },
        )]);
        let missing_binding_error =
            check_module(&missing_binding).expect_err("record fields need bindings");
        assert!(
            missing_binding_error
                .message()
                .contains("missing a binding")
        );

        let incompatible = module(vec![binding(
            0,
            "destructured",
            hir::Expr::Block {
                items: vec![
                    hir::BlockItem::Binding(hir::BindingDecl {
                        pattern: hir::Pattern::Record {
                            fields: vec![hir::RecordPatternField {
                                name: "name".to_owned(),
                                binding: Some(binding_name(1, "name")),
                                pattern: None,
                                span: span(),
                            }],
                            span: span(),
                        },
                        value: number(1.0),
                        is_exported: false,
                        span: span(),
                    }),
                    hir::BlockItem::Expr(identifier(1, "name")),
                ],
                span: span(),
            },
        )]);
        let incompatible_error =
            check_module(&incompatible).expect_err("record patterns require records");
        assert!(
            incompatible_error
                .message()
                .contains("record pattern is not compatible")
        );
    }

    #[test]
    fn resolves_type_aliases_and_reports_invalid_generic_usage() {
        let alias_module = module(vec![
            hir::ModuleItem::Type(hir::TypeDecl {
                name: type_name(0, "Boxed"),
                type_params: vec![type_param(0, "T")],
                value: record_type(vec![(
                    "value",
                    hir::TypeExpr::Reference {
                        reference: hir::TypeReference::TypeParam(type_param(0, "T")),
                        span: span(),
                    },
                )]),
                is_exported: false,
                span: span(),
            }),
            binding(
                1,
                "unbox",
                function(
                    vec![parameter(
                        2,
                        "value",
                        Some(hir::TypeExpr::Apply {
                            callee: hir::TypeReference::Alias(type_name(0, "Boxed")),
                            args: vec![primitive_type(hir::BuiltinPrimitive::Number)],
                            span: span(),
                        }),
                    )],
                    Some(primitive_type(hir::BuiltinPrimitive::Number)),
                    hir::Expr::Member {
                        object: Box::new(identifier(2, "value")),
                        property: "value".to_owned(),
                        span: span(),
                    },
                    false,
                ),
            ),
        ]);
        check_module(&alias_module).expect("well-formed aliases should typecheck");

        let bad_builtin_generic = module(vec![binding(
            0,
            "identity",
            function(
                vec![parameter(
                    1,
                    "value",
                    Some(hir::TypeExpr::Reference {
                        reference: hir::TypeReference::BuiltinGeneric(
                            hir::BuiltinGeneric::Sequence,
                        ),
                        span: span(),
                    }),
                )],
                None,
                identifier(1, "value"),
                false,
            ),
        )]);
        let bad_builtin_error =
            check_module(&bad_builtin_generic).expect_err("bare generic built-ins should fail");
        assert!(
            bad_builtin_error
                .message()
                .contains("generic built-in types must include type arguments")
        );

        let bad_result_arity = module(vec![binding(
            0,
            "identity",
            function(
                vec![parameter(
                    1,
                    "value",
                    Some(result_type(
                        number_type(),
                        primitive_type(hir::BuiltinPrimitive::String),
                    )),
                )],
                None,
                identifier(1, "value"),
                false,
            ),
        )]);
        check_module(&bad_result_arity).expect("Result with two arguments should typecheck");

        let wrong_alias_arity = module(vec![
            hir::ModuleItem::Type(hir::TypeDecl {
                name: type_name(0, "Boxed"),
                type_params: vec![type_param(0, "T")],
                value: record_type(vec![(
                    "value",
                    hir::TypeExpr::Reference {
                        reference: hir::TypeReference::TypeParam(type_param(0, "T")),
                        span: span(),
                    },
                )]),
                is_exported: false,
                span: span(),
            }),
            binding(
                1,
                "invalid",
                function(
                    vec![parameter(
                        2,
                        "value",
                        Some(hir::TypeExpr::Apply {
                            callee: hir::TypeReference::Alias(type_name(0, "Boxed")),
                            args: Vec::new(),
                            span: span(),
                        }),
                    )],
                    None,
                    identifier(2, "value"),
                    false,
                ),
            ),
        ]);
        let wrong_alias_error =
            check_module(&wrong_alias_arity).expect_err("wrong alias arity should fail");
        assert!(
            wrong_alias_error
                .message()
                .contains("expects 1 type arguments but received 0")
        );
    }

    fn number_type() -> hir::TypeExpr {
        primitive_type(hir::BuiltinPrimitive::Number)
    }
}
