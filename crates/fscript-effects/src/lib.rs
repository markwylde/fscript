//! Effect analysis for the current semantic slice.

use std::collections::BTreeMap;

use fscript_hir as hir;
use fscript_source::Span;
use thiserror::Error;

/// Analyzes a lowered module and infers the current callable effect metadata.
pub fn analyze_module(module: &hir::Module) -> Result<ModuleEffects, EffectError> {
    Analyzer::default().analyze_module(module)
}

/// Effect classification used by the current semantic frontend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    Pure,
    Deferred,
    Effectful,
}

impl Effect {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Effectful, _) | (_, Self::Effectful) => Self::Effectful,
            (Self::Deferred, _) | (_, Self::Deferred) => Self::Deferred,
            _ => Self::Pure,
        }
    }
}

/// Exported callable effects discovered while analyzing a module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModuleEffects {
    pub exports: Vec<ExportEffect>,
}

/// Effect metadata for one exported callable binding.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportEffect {
    pub name: String,
    pub effect: Effect,
    pub span: Span,
}

/// Effect-analysis failure.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct EffectError {
    message: String,
    span: Span,
}

impl EffectError {
    /// Returns the diagnostic message.
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValueInfo {
    immediate: Effect,
    kind: ValueKind,
}

impl ValueInfo {
    fn pure_value() -> Self {
        Self {
            immediate: Effect::Pure,
            kind: ValueKind::Value,
        }
    }

    fn module(module: StdModule) -> Self {
        Self {
            immediate: Effect::Pure,
            kind: ValueKind::Module(module),
        }
    }

    fn callable(callable: CallableInfo) -> Self {
        Self {
            immediate: Effect::Pure,
            kind: ValueKind::Callable(callable),
        }
    }

    fn for_binding(&self) -> Self {
        Self {
            immediate: Effect::Pure,
            kind: self.kind.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ValueKind {
    Value,
    Module(StdModule),
    Callable(CallableInfo),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CallableInfo {
    arity: usize,
    effect: Effect,
    result_kind: Box<ValueKind>,
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

#[derive(Default)]
struct Analyzer {
    scopes: Vec<BTreeMap<hir::BindingId, ValueInfo>>,
    exports: Vec<ExportEffect>,
}

impl Analyzer {
    fn analyze_module(mut self, module: &hir::Module) -> Result<ModuleEffects, EffectError> {
        self.push_scope();

        for item in &module.items {
            match item {
                hir::ModuleItem::Import(import) => self.bind_import(import)?,
                hir::ModuleItem::Type(_) => {}
                hir::ModuleItem::Binding(binding) => {
                    let value = self.analyze_binding(binding)?;
                    self.record_export(binding, &value);
                }
            }
        }

        self.pop_scope();

        Ok(ModuleEffects {
            exports: self.exports,
        })
    }

    fn bind_import(&mut self, import: &hir::ImportDecl) -> Result<(), EffectError> {
        if let Some(module) = std_module_from_source(&import.source) {
            match &import.clause {
                hir::ImportClause::Default(binding) => {
                    self.bind(binding.id, ValueInfo::module(module));
                }
                hir::ImportClause::Named(bindings) => {
                    for binding in bindings {
                        self.bind(
                            binding.id,
                            ValueInfo {
                                immediate: Effect::Pure,
                                kind: module_export_kind(module, &binding.name, binding.span)?,
                            },
                        );
                    }
                }
            }
        } else {
            match &import.clause {
                hir::ImportClause::Default(binding) => {
                    self.bind(binding.id, ValueInfo::pure_value());
                }
                hir::ImportClause::Named(bindings) => {
                    for binding in bindings {
                        self.bind(binding.id, ValueInfo::pure_value());
                    }
                }
            }
        }

        Ok(())
    }

    fn analyze_binding(&mut self, binding: &hir::BindingDecl) -> Result<ValueInfo, EffectError> {
        let value = self.analyze_expr(&binding.value)?;
        self.bind_pattern(&binding.pattern, &value.for_binding());
        Ok(value)
    }

    fn analyze_expr(&mut self, expr: &hir::Expr) -> Result<ValueInfo, EffectError> {
        match expr {
            hir::Expr::StringLiteral { .. }
            | hir::Expr::NumberLiteral { .. }
            | hir::Expr::BooleanLiteral { .. }
            | hir::Expr::Null { .. }
            | hir::Expr::Undefined { .. } => Ok(ValueInfo::pure_value()),
            hir::Expr::Identifier(identifier) => self.lookup(identifier.id, identifier.span),
            hir::Expr::Record { fields, .. } => {
                let mut effect = Effect::Pure;
                for field in fields {
                    effect = effect.join(self.analyze_expr(&field.value)?.immediate);
                }
                Ok(ValueInfo {
                    immediate: effect,
                    kind: ValueKind::Value,
                })
            }
            hir::Expr::Array { items, .. } => {
                let mut effect = Effect::Pure;
                for item in items {
                    effect = effect.join(self.analyze_expr(item)?.immediate);
                }
                Ok(ValueInfo {
                    immediate: effect,
                    kind: ValueKind::Value,
                })
            }
            hir::Expr::Function {
                parameters,
                body,
                is_generator,
                span,
                ..
            } => self.analyze_function(parameters, body, *is_generator, *span),
            hir::Expr::Block { items, .. } => self.analyze_block(items),
            hir::Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                let condition = self.analyze_expr(condition)?;
                let then_branch = self.analyze_expr(then_branch)?;
                let else_branch = if let Some(else_branch) = else_branch {
                    self.analyze_expr(else_branch)?
                } else {
                    ValueInfo::pure_value()
                };

                Ok(ValueInfo {
                    immediate: condition
                        .immediate
                        .join(then_branch.immediate.join(else_branch.immediate)),
                    kind: merge_value_kinds(then_branch.kind, else_branch.kind),
                })
            }
            hir::Expr::Match { value, arms, span } => {
                let value = self.analyze_expr(value)?;
                let mut effect = value.immediate;
                let mut result_kind = None;

                for arm in arms {
                    self.push_scope();
                    self.bind_pattern(&arm.pattern, &ValueInfo::pure_value());
                    let body = self.analyze_expr(&arm.body)?;
                    self.pop_scope();

                    effect = effect.join(body.immediate);
                    result_kind = Some(match result_kind {
                        Some(previous) => merge_value_kinds(previous, body.kind),
                        None => body.kind,
                    });
                }

                if arms.is_empty() {
                    return Err(EffectError {
                        message: "match expressions must contain at least one arm".to_owned(),
                        span: *span,
                    });
                }

                Ok(ValueInfo {
                    immediate: effect,
                    kind: result_kind.unwrap_or(ValueKind::Value),
                })
            }
            hir::Expr::Try {
                body,
                catch_pattern,
                catch_body,
                ..
            } => {
                let body = self.analyze_expr(body)?;

                self.push_scope();
                self.bind_pattern(catch_pattern, &ValueInfo::pure_value());
                let catch_body = self.analyze_expr(catch_body)?;
                self.pop_scope();

                Ok(ValueInfo {
                    immediate: body.immediate.join(catch_body.immediate),
                    kind: merge_value_kinds(body.kind, catch_body.kind),
                })
            }
            hir::Expr::Throw { value, .. } => {
                let value = self.analyze_expr(value)?;
                Ok(ValueInfo {
                    immediate: value.immediate.join(Effect::Effectful),
                    kind: ValueKind::Value,
                })
            }
            hir::Expr::Yield { value, span } => {
                let value = self.analyze_expr(value)?;
                if value.immediate != Effect::Pure {
                    return Err(EffectError {
                        message: "generator yields must remain pure in the current effect slice"
                            .to_owned(),
                        span: *span,
                    });
                }
                Ok(ValueInfo::pure_value())
            }
            hir::Expr::Unary {
                operator, operand, ..
            } => match operator {
                hir::UnaryOperator::Defer => {
                    let mut operand = self.analyze_expr(operand)?;
                    operand.immediate = Effect::Deferred;
                    Ok(operand)
                }
                hir::UnaryOperator::Not
                | hir::UnaryOperator::Negate
                | hir::UnaryOperator::Positive => {
                    let operand = self.analyze_expr(operand)?;
                    Ok(ValueInfo {
                        immediate: operand.immediate,
                        kind: ValueKind::Value,
                    })
                }
            },
            hir::Expr::Binary { left, right, .. } => {
                let left = self.analyze_expr(left)?;
                let right = self.analyze_expr(right)?;
                Ok(ValueInfo {
                    immediate: left.immediate.join(right.immediate),
                    kind: ValueKind::Value,
                })
            }
            hir::Expr::Call { callee, args, .. } => self.analyze_call(callee, args),
            hir::Expr::Member {
                object,
                property,
                span,
            } => {
                let object = self.analyze_expr(object)?;
                let kind = if let ValueKind::Module(module) = object.kind {
                    module_export_kind(module, property, *span)?
                } else {
                    ValueKind::Value
                };

                Ok(ValueInfo {
                    immediate: object.immediate,
                    kind,
                })
            }
            hir::Expr::Index { object, index, .. } => {
                let object = self.analyze_expr(object)?;
                let index = self.analyze_expr(index)?;
                Ok(ValueInfo {
                    immediate: object.immediate.join(index.immediate),
                    kind: ValueKind::Value,
                })
            }
        }
    }

    fn analyze_function(
        &mut self,
        parameters: &[hir::Parameter],
        body: &hir::Expr,
        is_generator: bool,
        span: Span,
    ) -> Result<ValueInfo, EffectError> {
        self.push_scope();
        for parameter in parameters {
            self.bind_pattern(&parameter.pattern, &ValueInfo::pure_value());
        }
        let body = self.analyze_expr(body)?;
        self.pop_scope();

        if is_generator && body.immediate != Effect::Pure {
            return Err(EffectError {
                message: "generator bodies must remain pure in the current effect slice".to_owned(),
                span,
            });
        }

        let effect = if is_generator {
            Effect::Pure
        } else {
            body.immediate
        };

        Ok(ValueInfo::callable(CallableInfo {
            arity: parameters.len(),
            effect,
            result_kind: Box::new(if is_generator {
                ValueKind::Value
            } else {
                body.kind
            }),
        }))
    }

    fn analyze_block(&mut self, items: &[hir::BlockItem]) -> Result<ValueInfo, EffectError> {
        self.push_scope();

        let mut effect = Effect::Pure;
        let mut result_kind = ValueKind::Value;

        for item in items {
            match item {
                hir::BlockItem::Binding(binding) => {
                    let value = self.analyze_binding(binding)?;
                    effect = effect.join(value.immediate);
                    result_kind = ValueKind::Value;
                }
                hir::BlockItem::Expr(expr) => {
                    let value = self.analyze_expr(expr)?;
                    effect = effect.join(value.immediate);
                    result_kind = value.kind;
                }
            }
        }

        self.pop_scope();

        Ok(ValueInfo {
            immediate: effect,
            kind: result_kind,
        })
    }

    fn analyze_call(
        &mut self,
        callee: &hir::Expr,
        args: &[hir::Expr],
    ) -> Result<ValueInfo, EffectError> {
        let callee = self.analyze_expr(callee)?;
        let mut effect = callee.immediate;
        for arg in args {
            effect = effect.join(self.analyze_expr(arg)?.immediate);
        }

        let ValueKind::Callable(callable) = callee.kind else {
            return Ok(ValueInfo {
                immediate: effect,
                kind: ValueKind::Value,
            });
        };

        if args.len() < callable.arity {
            return Ok(ValueInfo::callable(CallableInfo {
                arity: callable.arity - args.len(),
                effect: callable.effect,
                result_kind: callable.result_kind,
            }));
        }

        Ok(ValueInfo {
            immediate: effect.join(callable.effect),
            kind: *callable.result_kind,
        })
    }

    fn record_export(&mut self, binding: &hir::BindingDecl, value: &ValueInfo) {
        if !binding.is_exported {
            return;
        }

        let hir::Pattern::Identifier(name) = &binding.pattern else {
            return;
        };

        let ValueKind::Callable(callable) = &value.kind else {
            return;
        };

        self.exports.push(ExportEffect {
            name: name.name.clone(),
            effect: callable.effect,
            span: name.span,
        });
    }

    fn bind_pattern(&mut self, pattern: &hir::Pattern, value: &ValueInfo) {
        match pattern {
            hir::Pattern::Identifier(binding) => self.bind(binding.id, value.clone()),
            hir::Pattern::Literal(_) => {}
            hir::Pattern::Array { items, .. } => {
                for item in items {
                    self.bind_pattern(item, &ValueInfo::pure_value());
                }
            }
            hir::Pattern::Record { fields, .. } => {
                for field in fields {
                    if let Some(pattern) = &field.pattern {
                        self.bind_pattern(pattern, &ValueInfo::pure_value());
                    } else if let Some(binding) = &field.binding {
                        self.bind(binding.id, ValueInfo::pure_value());
                    }
                }
            }
        }
    }

    fn lookup(&self, binding_id: hir::BindingId, span: Span) -> Result<ValueInfo, EffectError> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(&binding_id))
            .cloned()
            .ok_or_else(|| EffectError {
                message: "unknown identifier".to_owned(),
                span,
            })
    }

    fn bind(&mut self, binding_id: hir::BindingId, value: ValueInfo) {
        self.scopes
            .last_mut()
            .expect("effect scopes always include a root scope")
            .insert(binding_id, value);
    }

    fn push_scope(&mut self) {
        self.scopes.push(BTreeMap::new());
    }

    fn pop_scope(&mut self) {
        let popped = self.scopes.pop();
        debug_assert!(popped.is_some());
    }
}

fn merge_value_kinds(left: ValueKind, right: ValueKind) -> ValueKind {
    match (left, right) {
        (ValueKind::Module(left), ValueKind::Module(right)) if left == right => {
            ValueKind::Module(left)
        }
        (ValueKind::Callable(left), ValueKind::Callable(right)) if left.arity == right.arity => {
            ValueKind::Callable(CallableInfo {
                arity: left.arity,
                effect: left.effect.join(right.effect),
                result_kind: Box::new(merge_value_kinds(*left.result_kind, *right.result_kind)),
            })
        }
        _ => ValueKind::Value,
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

fn module_export_kind(module: StdModule, name: &str, span: Span) -> Result<ValueKind, EffectError> {
    let callable = match (module, name) {
        (StdModule::Array, "map") | (StdModule::Array, "filter") => CallableInfo {
            arity: 2,
            effect: Effect::Pure,
            result_kind: Box::new(ValueKind::Value),
        },
        (StdModule::Array, "length")
        | (StdModule::Json, "parse")
        | (StdModule::Json, "stringify")
        | (StdModule::Json, "jsonToObject")
        | (StdModule::Json, "jsonToString")
        | (StdModule::Json, "jsonToPrettyString")
        | (StdModule::String, "trim")
        | (StdModule::String, "uppercase")
        | (StdModule::String, "lowercase")
        | (StdModule::String, "isDigits")
        | (StdModule::Number, "parse")
        | (StdModule::Result, "ok")
        | (StdModule::Result, "error")
        | (StdModule::Result, "isOk")
        | (StdModule::Result, "isError")
        | (StdModule::Filesystem, "readFile")
        | (StdModule::Filesystem, "exists")
        | (StdModule::Filesystem, "deleteFile")
        | (StdModule::Filesystem, "readDir") => CallableInfo {
            arity: 1,
            effect: if matches!(module, StdModule::Filesystem) {
                Effect::Effectful
            } else {
                Effect::Pure
            },
            result_kind: Box::new(ValueKind::Value),
        },
        (StdModule::Object, "spread")
        | (StdModule::Result, "withDefault")
        | (StdModule::Logger, "create")
        | (StdModule::Logger, "log")
        | (StdModule::Logger, "debug")
        | (StdModule::Logger, "info")
        | (StdModule::Logger, "warn")
        | (StdModule::Logger, "error")
        | (StdModule::Logger, "prettyJson")
        | (StdModule::Filesystem, "writeFile")
        | (StdModule::Http, "serve") => CallableInfo {
            arity: match (module, name) {
                (StdModule::Object, "spread")
                | (StdModule::Result, "withDefault")
                | (StdModule::Filesystem, "writeFile")
                | (StdModule::Http, "serve")
                | (StdModule::Logger, "log")
                | (StdModule::Logger, "debug")
                | (StdModule::Logger, "info")
                | (StdModule::Logger, "warn")
                | (StdModule::Logger, "error")
                | (StdModule::Logger, "prettyJson") => 2,
                (StdModule::Logger, "create") => 1,
                _ => 2,
            },
            effect: if matches!(
                module,
                StdModule::Filesystem | StdModule::Http | StdModule::Logger
            ) {
                Effect::Effectful
            } else {
                Effect::Pure
            },
            result_kind: Box::new(ValueKind::Value),
        },
        (StdModule::Task, "all")
        | (StdModule::Task, "race")
        | (StdModule::Task, "spawn")
        | (StdModule::Task, "defer")
        | (StdModule::Task, "force") => CallableInfo {
            arity: 1,
            effect: Effect::Deferred,
            result_kind: Box::new(ValueKind::Value),
        },
        _ => {
            return Err(EffectError {
                message: format!(
                    "module `{}` does not export `{name}`",
                    std_module_name(module)
                ),
                span,
            });
        }
    };

    Ok(ValueKind::Callable(callable))
}

fn std_module_name(module: StdModule) -> &'static str {
    match module {
        StdModule::Array => "std:array",
        StdModule::Object => "std:object",
        StdModule::String => "std:string",
        StdModule::Number => "std:number",
        StdModule::Result => "std:result",
        StdModule::Json => "std:json",
        StdModule::Logger => "std:logger",
        StdModule::Http => "std:http",
        StdModule::Filesystem => "std:filesystem",
        StdModule::Task => "std:task",
    }
}

#[cfg(test)]
mod tests {
    use super::{Effect, analyze_module};
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

    fn identifier(id: u32, name: &str) -> hir::Expr {
        hir::Expr::Identifier(name_ref(id, name))
    }

    fn string(value: &str) -> hir::Expr {
        hir::Expr::StringLiteral {
            value: value.to_owned(),
            span: span(),
        }
    }

    fn empty_record() -> hir::Expr {
        hir::Expr::Record {
            fields: Vec::new(),
            span: span(),
        }
    }

    fn parameter(id: u32, name: &str) -> hir::Parameter {
        hir::Parameter {
            pattern: hir::Pattern::Identifier(binding_name(id, name)),
            type_annotation: None,
            span: span(),
        }
    }

    fn import_default(id: u32, source: &str) -> hir::ModuleItem {
        hir::ModuleItem::Import(hir::ImportDecl {
            clause: hir::ImportClause::Default(binding_name(id, source)),
            source: source.to_owned(),
            source_span: span(),
            span: span(),
        })
    }

    fn exported_binding(id: u32, name: &str, value: hir::Expr) -> hir::ModuleItem {
        hir::ModuleItem::Binding(hir::BindingDecl {
            pattern: hir::Pattern::Identifier(binding_name(id, name)),
            value,
            is_exported: true,
            span: span(),
        })
    }

    #[test]
    fn reports_exported_pure_function_effects() {
        let module = hir::Module {
            items: vec![exported_binding(
                0,
                "identity",
                hir::Expr::Function {
                    parameters: vec![parameter(1, "value")],
                    return_type: None,
                    body: Box::new(identifier(1, "value")),
                    is_generator: false,
                    span: span(),
                },
            )],
        };

        let effects = analyze_module(&module).expect("pure function should analyze");

        assert_eq!(effects.exports.len(), 1);
        assert_eq!(effects.exports[0].name, "identity");
        assert_eq!(effects.exports[0].effect, Effect::Pure);
    }

    #[test]
    fn keeps_partial_application_callable_effects() {
        let object_import_id = 0;
        let module = hir::Module {
            items: vec![
                import_default(object_import_id, "std:object"),
                exported_binding(
                    1,
                    "merge_active",
                    hir::Expr::Call {
                        callee: Box::new(hir::Expr::Member {
                            object: Box::new(identifier(object_import_id, "std:object")),
                            property: "spread".to_owned(),
                            span: span(),
                        }),
                        args: vec![empty_record()],
                        span: span(),
                    },
                ),
            ],
        };

        let effects = analyze_module(&module).expect("partial application should analyze");

        assert_eq!(effects.exports.len(), 1);
        assert_eq!(effects.exports[0].name, "merge_active");
        assert_eq!(effects.exports[0].effect, Effect::Pure);
    }

    #[test]
    fn reports_deferred_exported_functions() {
        let filesystem_import_id = 0;
        let module = hir::Module {
            items: vec![
                import_default(filesystem_import_id, "std:filesystem"),
                exported_binding(
                    1,
                    "load_later",
                    hir::Expr::Function {
                        parameters: vec![parameter(2, "path")],
                        return_type: None,
                        body: Box::new(hir::Expr::Unary {
                            operator: hir::UnaryOperator::Defer,
                            operand: Box::new(hir::Expr::Call {
                                callee: Box::new(hir::Expr::Member {
                                    object: Box::new(identifier(
                                        filesystem_import_id,
                                        "std:filesystem",
                                    )),
                                    property: "readFile".to_owned(),
                                    span: span(),
                                }),
                                args: vec![identifier(2, "path")],
                                span: span(),
                            }),
                            span: span(),
                        }),
                        is_generator: false,
                        span: span(),
                    },
                ),
            ],
        };

        let effects = analyze_module(&module).expect("deferred function should analyze");

        assert_eq!(effects.exports.len(), 1);
        assert_eq!(effects.exports[0].name, "load_later");
        assert_eq!(effects.exports[0].effect, Effect::Deferred);
    }

    #[test]
    fn rejects_effectful_generators() {
        let filesystem_import_id = 0;
        let module = hir::Module {
            items: vec![
                import_default(filesystem_import_id, "std:filesystem"),
                exported_binding(
                    1,
                    "lines",
                    hir::Expr::Function {
                        parameters: vec![parameter(2, "path")],
                        return_type: None,
                        body: Box::new(hir::Expr::Yield {
                            value: Box::new(hir::Expr::Call {
                                callee: Box::new(hir::Expr::Member {
                                    object: Box::new(identifier(
                                        filesystem_import_id,
                                        "std:filesystem",
                                    )),
                                    property: "readFile".to_owned(),
                                    span: span(),
                                }),
                                args: vec![string("notes.txt")],
                                span: span(),
                            }),
                            span: span(),
                        }),
                        is_generator: true,
                        span: span(),
                    },
                ),
            ],
        };

        let error = analyze_module(&module).expect_err("effectful generators must fail");

        assert_eq!(
            error.message(),
            "generator yields must remain pure in the current effect slice"
        );
        assert_eq!(error.span(), span());
    }
}
