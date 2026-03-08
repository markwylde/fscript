//! IR interpreter for the first shared execution slice.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use fscript_ir as ir;
use fscript_runtime::{
    DeferredBody, DeferredOutcome, DeferredValue, Environment, FunctionValue, NativeFunctionValue,
    RuntimeError, SchedulerExecutor, SingleThreadedScheduler, Value,
};

/// Executes a lowered IR module and returns the final top-level value.
pub fn run_module(module: &ir::Module) -> Result<Option<Value>, RuntimeError> {
    let modules = BTreeMap::from([(ENTRY_MODULE.to_owned(), module.clone())]);
    run_program(&modules, ENTRY_MODULE)
}

/// Executes a graph of lowered IR modules rooted at the provided entry key.
pub fn run_program(
    modules: &BTreeMap<String, ir::Module>,
    entry: &str,
) -> Result<Option<Value>, RuntimeError> {
    let mut cache = BTreeMap::new();
    let mut active = BTreeSet::new();
    execute_module(entry, modules, &mut cache, &mut active).map(|module| module.last_value)
}

type YieldValues<'a> = Option<&'a RefCell<Vec<Value>>>;
type RuntimeEval<T> = Result<T, RuntimeControl>;

const ENTRY_MODULE: &str = "<entry>";

#[derive(Clone, Debug, PartialEq)]
enum EvalOutcome {
    Value(Value),
    Throw(Value),
}

#[derive(Debug)]
enum RuntimeControl {
    Error(RuntimeError),
    Throw(Value),
}

#[derive(Clone, Debug, PartialEq)]
struct ExecutedModule {
    exports: BTreeMap<String, Value>,
    last_value: Option<Value>,
}

impl From<RuntimeError> for RuntimeControl {
    fn from(value: RuntimeError) -> Self {
        Self::Error(value)
    }
}

fn execute_module(
    module_id: &str,
    modules: &BTreeMap<String, ir::Module>,
    cache: &mut BTreeMap<String, ExecutedModule>,
    active: &mut BTreeSet<String>,
) -> Result<ExecutedModule, RuntimeError> {
    if let Some(module) = cache.get(module_id) {
        return Ok(module.clone());
    }

    let Some(module) = modules.get(module_id) else {
        return Err(RuntimeError::new(format!(
            "runtime module `{module_id}` is not available"
        )));
    };

    if !active.insert(module_id.to_owned()) {
        return Err(RuntimeError::new(format!(
            "circular runtime module dependency involving `{module_id}`"
        )));
    }

    let mut environment = Environment::new();
    let mut last_value = None;

    for item in &module.items {
        match item {
            ir::ModuleItem::Import(import) => {
                load_import(import, &mut environment, modules, cache, active)?
            }
            ir::ModuleItem::Binding(binding) => {
                let value = evaluate_expr(&binding.value, &environment)?;
                bind_pattern_value(&mut environment, &binding.pattern, value.clone(), None)
                    .map_err(runtime_control_to_error)?;
                last_value = Some(value);
            }
        }
    }

    let last_value = last_value
        .map(|value| force_value(value, None).map_err(runtime_control_to_error))
        .transpose()?;
    let exports = module
        .exports
        .iter()
        .map(|name| {
            environment
                .get(name)
                .cloned()
                .map(|value| (name.clone(), value))
                .ok_or_else(|| RuntimeError::new(format!("module export `{name}` is not defined")))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;

    let executed = ExecutedModule {
        exports,
        last_value,
    };
    cache.insert(module_id.to_owned(), executed.clone());
    active.remove(module_id);
    Ok(executed)
}

fn load_import(
    import: &ir::ImportDecl,
    environment: &mut Environment,
    modules: &BTreeMap<String, ir::Module>,
    cache: &mut BTreeMap<String, ExecutedModule>,
    active: &mut BTreeSet<String>,
) -> Result<(), RuntimeError> {
    let module = if import.source.starts_with("std:") {
        fscript_std::load_module(&import.source)?
    } else {
        Value::Record(execute_module(&import.source, modules, cache, active)?.exports)
    };

    match &import.clause {
        ir::ImportClause::Default(identifier) => define_binding(environment, identifier, module),
        ir::ImportClause::Named(names) => {
            let Value::Record(exports) = &module else {
                return Err(RuntimeError::new(format!(
                    "module `{}` does not expose named exports",
                    import.source
                )));
            };

            for name in names {
                let value = exports.get(name).cloned().ok_or_else(|| {
                    RuntimeError::new(format!(
                        "module `{}` does not export `{name}`",
                        import.source
                    ))
                })?;
                define_binding(environment, name, value)?;
            }

            Ok(())
        }
    }
}

fn define_binding(
    environment: &mut Environment,
    name: &str,
    value: Value,
) -> Result<(), RuntimeError> {
    if environment.contains_key(name) {
        return Err(RuntimeError::new(format!(
            "binding `{name}` is already defined in this scope"
        )));
    }

    environment.insert(name.to_owned(), value);
    Ok(())
}

fn bind_pattern_value(
    environment: &mut Environment,
    pattern: &ir::Pattern,
    value: Value,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<()> {
    let value = prepare_pattern_value(pattern, value, yield_values)?;
    let Some(bindings) = match_pattern(pattern, &value)? else {
        return Err(RuntimeControl::from(RuntimeError::new(format!(
            "value `{value}` does not match binding pattern"
        ))));
    };

    for (name, binding_value) in bindings {
        define_binding(environment, &name, binding_value).map_err(RuntimeControl::from)?;
    }

    Ok(())
}

fn match_pattern(
    pattern: &ir::Pattern,
    value: &Value,
) -> Result<Option<Environment>, RuntimeControl> {
    let mut bindings = Environment::new();
    if pattern_matches(pattern, value, &mut bindings)? {
        Ok(Some(bindings))
    } else {
        Ok(None)
    }
}

fn pattern_matches(
    pattern: &ir::Pattern,
    value: &Value,
    bindings: &mut Environment,
) -> Result<bool, RuntimeControl> {
    match pattern {
        ir::Pattern::Identifier { name, .. } => {
            if bindings.contains_key(name) {
                return Err(RuntimeError::new(format!(
                    "binding `{name}` is already defined in this scope"
                ))
                .into());
            }

            bindings.insert(name.clone(), value.clone());
            Ok(true)
        }
        ir::Pattern::Literal(literal) => Ok(match literal {
            ir::LiteralPattern::String {
                value: expected, ..
            } => {
                matches!(value, Value::String(actual) if actual == expected)
            }
            ir::LiteralPattern::Number {
                value: expected, ..
            } => {
                matches!(value, Value::Number(actual) if actual == expected)
            }
            ir::LiteralPattern::Boolean {
                value: expected, ..
            } => {
                matches!(value, Value::Boolean(actual) if actual == expected)
            }
            ir::LiteralPattern::Null { .. } => matches!(value, Value::Null),
            ir::LiteralPattern::Undefined { .. } => matches!(value, Value::Undefined),
        }),
        ir::Pattern::Record { fields, .. } => {
            let Value::Record(record) = value else {
                return Ok(false);
            };

            for field in fields {
                let Some(field_value) = record.get(&field.name) else {
                    return Ok(false);
                };

                if let Some(pattern) = &field.pattern {
                    if !pattern_matches(pattern, field_value, bindings)? {
                        return Ok(false);
                    }
                } else {
                    let binding = field.binding.clone().unwrap_or_else(|| field.name.clone());
                    if bindings
                        .insert(binding.clone(), field_value.clone())
                        .is_some()
                    {
                        return Err(RuntimeError::new(format!(
                            "binding `{binding}` is already defined in this scope"
                        ))
                        .into());
                    }
                }
            }

            Ok(true)
        }
        ir::Pattern::Array { items, .. } => {
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

fn evaluate_expr(expr: &ir::Expr, environment: &Environment) -> Result<Value, RuntimeError> {
    match evaluate_expr_with_yields(expr, environment, None) {
        Ok(EvalOutcome::Value(value)) => Ok(value),
        Ok(EvalOutcome::Throw(value)) | Err(RuntimeControl::Throw(value)) => Err(
            RuntimeError::new(format!("uncaught thrown value `{value}`")),
        ),
        Err(RuntimeControl::Error(error)) => Err(error),
    }
}

fn evaluate_expr_with_yields(
    expr: &ir::Expr,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<EvalOutcome> {
    Ok(match expr {
        ir::Expr::StringLiteral { value, .. } => EvalOutcome::Value(Value::String(value.clone())),
        ir::Expr::NumberLiteral { value, .. } => EvalOutcome::Value(Value::Number(*value)),
        ir::Expr::BooleanLiteral { value, .. } => EvalOutcome::Value(Value::Boolean(*value)),
        ir::Expr::Null { .. } => EvalOutcome::Value(Value::Null),
        ir::Expr::Undefined { .. } => EvalOutcome::Value(Value::Undefined),
        ir::Expr::Identifier { name, .. } => EvalOutcome::Value(
            environment
                .get(name)
                .cloned()
                .ok_or_else(|| RuntimeError::new(format!("unknown identifier `{name}`")))
                .map_err(RuntimeControl::from)?,
        ),
        ir::Expr::Record { fields, .. } => EvalOutcome::Value(Value::Record(
            evaluate_record_fields(fields, environment, yield_values)?,
        )),
        ir::Expr::Array { items, .. } => EvalOutcome::Value(Value::Array(
            items
                .iter()
                .map(|item| evaluate_value_expr(item, environment, yield_values))
                .collect::<RuntimeEval<Vec<_>>>()?,
        )),
        ir::Expr::Function {
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
        ir::Expr::Block { items, .. } => {
            let mut block_environment = environment.clone();
            let mut last_value = Value::Undefined;

            for item in items {
                match item {
                    ir::BlockItem::Binding(binding) => {
                        let value =
                            evaluate_value_expr(&binding.value, &block_environment, yield_values)?;
                        bind_pattern_value(
                            &mut block_environment,
                            &binding.pattern,
                            value,
                            yield_values,
                        )?;
                    }
                    ir::BlockItem::Expr(expr) => {
                        last_value = evaluate_value_expr(expr, &block_environment, yield_values)?;
                    }
                }
            }

            EvalOutcome::Value(last_value)
        }
        ir::Expr::If {
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
                return Err(RuntimeControl::Error(RuntimeError::new(format!(
                    "`if` conditions must evaluate to Boolean values, found `{other}`"
                ))));
            }
        },
        ir::Expr::Match { value, arms, .. } => {
            let value = consume_expr_value(value, environment, yield_values)?;
            evaluate_match_arms(arms, value, environment, yield_values)?
        }
        ir::Expr::Try {
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
        ir::Expr::Throw { value, .. } => {
            EvalOutcome::Throw(evaluate_value_expr(value, environment, yield_values)?)
        }
        ir::Expr::Yield { value, .. } => {
            let Some(yield_values) = yield_values else {
                return Err(RuntimeError::new("runtime support for `yield` outside generator execution is not implemented yet").into());
            };
            let value = evaluate_value_expr(value, environment, Some(yield_values))?;
            yield_values.borrow_mut().push(value.clone());
            EvalOutcome::Value(value)
        }
        ir::Expr::Unary {
            operator, operand, ..
        } => match operator {
            ir::UnaryOperator::Defer => {
                EvalOutcome::Value(Value::Deferred(DeferredValue::new(DeferredBody::Expr {
                    expr: operand.clone(),
                    environment: environment.clone(),
                })))
            }
            _ => {
                let operand = consume_expr_value(operand, environment, yield_values)?;
                EvalOutcome::Value(evaluate_unary_expr(*operator, operand)?)
            }
        },
        ir::Expr::Binary {
            operator,
            left,
            right,
            ..
        } => {
            let left = consume_expr_value(left, environment, yield_values)?;
            let right = consume_expr_value(right, environment, yield_values)?;
            EvalOutcome::Value(evaluate_binary_expr(*operator, left, right)?)
        }
        ir::Expr::Call { callee, args, .. } => {
            let callee = evaluate_value_expr(callee, environment, yield_values)?;
            let args = args
                .iter()
                .map(|arg| evaluate_value_expr(arg, environment, yield_values))
                .collect::<RuntimeEval<Vec<_>>>()?;
            EvalOutcome::Value(call_value(callee, args, yield_values)?)
        }
        ir::Expr::Member {
            object, property, ..
        } => {
            let object = consume_expr_value(object, environment, yield_values)?;
            let Value::Record(fields) = object else {
                return Err(RuntimeError::new(format!(
                    "cannot read property `{property}` from non-record value"
                ))
                .into());
            };

            EvalOutcome::Value(
                fields
                    .get(property)
                    .cloned()
                    .ok_or_else(|| {
                        RuntimeError::new(format!("record does not contain a `{property}` field"))
                    })
                    .map_err(RuntimeControl::from)?,
            )
        }
        ir::Expr::Index { object, index, .. } => {
            let object = consume_expr_value(object, environment, yield_values)?;
            let index = consume_expr_value(index, environment, yield_values)?;
            EvalOutcome::Value(evaluate_index_expr(object, index)?)
        }
    })
}

fn evaluate_record_fields(
    fields: &[ir::RecordField],
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<BTreeMap<String, Value>> {
    let mut record = BTreeMap::new();

    for field in fields {
        let value = evaluate_value_expr(&field.value, environment, yield_values)?;
        record.insert(field.name.clone(), value);
    }

    Ok(record)
}

fn evaluate_match_arms(
    arms: &[ir::MatchArm],
    value: Value,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<EvalOutcome> {
    for arm in arms {
        if let Some(bindings) = match_pattern(&arm.pattern, &value)? {
            let mut arm_environment = environment.clone();
            for (name, binding_value) in bindings {
                define_binding(&mut arm_environment, &name, binding_value)
                    .map_err(RuntimeControl::from)?;
            }

            return evaluate_expr_with_yields(&arm.body, &arm_environment, yield_values);
        }
    }

    Err(RuntimeError::new("match expression did not find a matching arm").into())
}

fn evaluate_unary_expr(operator: ir::UnaryOperator, operand: Value) -> RuntimeEval<Value> {
    match operator {
        ir::UnaryOperator::Not => match operand {
            Value::Boolean(value) => Ok(Value::Boolean(!value)),
            other => Err(RuntimeError::new(format!("cannot apply `!` to value `{other}`")).into()),
        },
        ir::UnaryOperator::Negate => match operand {
            Value::Number(value) => Ok(Value::Number(-value)),
            other => Err(RuntimeError::new(format!("cannot negate value `{other}`")).into()),
        },
        ir::UnaryOperator::Positive => match operand {
            Value::Number(value) => Ok(Value::Number(value)),
            other => {
                Err(RuntimeError::new(format!("cannot apply unary `+` to value `{other}`")).into())
            }
        },
        ir::UnaryOperator::Defer => Err(RuntimeError::new(
            "runtime support for `defer` expressions is not implemented yet",
        )
        .into()),
    }
}

fn evaluate_value_expr(
    expr: &ir::Expr,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    match evaluate_expr_with_yields(expr, environment, yield_values)? {
        EvalOutcome::Value(value) => Ok(value),
        EvalOutcome::Throw(value) => Err(RuntimeControl::Throw(value)),
    }
}

fn consume_expr_value(
    expr: &ir::Expr,
    environment: &Environment,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    let value = evaluate_value_expr(expr, environment, yield_values)?;
    force_value(value, yield_values)
}

fn prepare_pattern_value(
    pattern: &ir::Pattern,
    value: Value,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    match pattern {
        ir::Pattern::Identifier { .. } => Ok(value),
        _ => force_value(value, yield_values),
    }
}

fn force_value(value: Value, yield_values: YieldValues<'_>) -> RuntimeEval<Value> {
    match value {
        Value::Deferred(deferred) => {
            if let Some(outcome) = deferred.outcome() {
                return resolve_deferred_outcome(outcome, yield_values);
            }

            let mut scheduler = SingleThreadedScheduler::new();
            let mut executor = InterpreterSchedulerExecutor { yield_values };
            let outcome = scheduler.force_deferred(deferred, &mut executor)?;
            resolve_deferred_outcome(outcome, yield_values)
        }
        other => Ok(other),
    }
}

fn resolve_deferred_outcome(
    outcome: DeferredOutcome,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    match outcome {
        DeferredOutcome::Value(value) => force_value(value, yield_values),
        DeferredOutcome::Throw(value) => Err(RuntimeControl::Throw(value)),
    }
}

fn start_deferred_value(value: &DeferredValue, yield_values: YieldValues<'_>) -> RuntimeEval<()> {
    if value.outcome().is_some() {
        return Ok(());
    }

    let mut scheduler = SingleThreadedScheduler::new();
    let mut executor = InterpreterSchedulerExecutor { yield_values };
    scheduler.start_deferred(value.clone(), &mut executor)?;
    Ok(())
}

struct InterpreterSchedulerExecutor<'a> {
    yield_values: YieldValues<'a>,
}

impl SchedulerExecutor<RuntimeControl> for InterpreterSchedulerExecutor<'_> {
    fn evaluate_expr_task(
        &mut self,
        expr: &ir::Expr,
        environment: &Environment,
    ) -> Result<DeferredOutcome, RuntimeControl> {
        match evaluate_expr_with_yields(expr, environment, self.yield_values)? {
            EvalOutcome::Value(value) => Ok(DeferredOutcome::Value(value)),
            EvalOutcome::Throw(value) => Ok(DeferredOutcome::Throw(value)),
        }
    }

    fn execute_native_task(
        &mut self,
        function: fscript_runtime::NativeFunction,
        args: Vec<Value>,
    ) -> Result<Value, RuntimeControl> {
        fscript_std::execute_native_function(
            function,
            args,
            |callee, args| call_value(callee, args, self.yield_values),
            |value| force_value(value, self.yield_values),
        )
    }

    fn force_task_input(&mut self, value: Value) -> Result<Value, RuntimeControl> {
        force_task_input(value, self.yield_values)
    }
}

fn runtime_control_to_error(control: RuntimeControl) -> RuntimeError {
    match control {
        RuntimeControl::Error(error) => error,
        RuntimeControl::Throw(value) => {
            RuntimeError::new(format!("uncaught thrown value `{value}`"))
        }
    }
}

fn evaluate_index_expr(object: Value, index: Value) -> RuntimeEval<Value> {
    let Value::Number(index) = index else {
        return Err(RuntimeError::new("array indexes must evaluate to numbers").into());
    };

    if index.is_sign_negative() || index.fract() != 0.0 {
        return Err(RuntimeError::new("array indexes must be non-negative whole numbers").into());
    }

    let index = index as usize;
    match object {
        Value::Array(items) | Value::Sequence(items) => {
            items.get(index).cloned().ok_or_else(|| {
                RuntimeControl::from(RuntimeError::new(format!(
                    "index `{index}` is out of bounds"
                )))
            })
        }
        other => Err(RuntimeError::new(format!("cannot index into value `{other}`")).into()),
    }
}

fn call_value(
    callee: Value,
    args: Vec<Value>,
    yield_values: YieldValues<'_>,
) -> RuntimeEval<Value> {
    let callee = force_value(callee, yield_values)?;

    match callee {
        Value::Function(function) => {
            let args = args
                .into_iter()
                .map(|arg| force_value(arg, yield_values))
                .collect::<RuntimeEval<Vec<_>>>()?;
            call_function(function, args, yield_values)
        }
        Value::NativeFunction(function) => call_native_function(function, args, yield_values),
        other => Err(RuntimeError::new(format!("cannot call value `{other}`")).into()),
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
        std::cmp::Ordering::Greater => Err(RuntimeError::new(format!(
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
            let function_id = function.function;
            let args = if function_id.forces_arguments() {
                all_args
                    .into_iter()
                    .map(|arg| force_value(arg, yield_values))
                    .collect::<RuntimeEval<Vec<_>>>()?
            } else {
                all_args
            };

            if function_id.is_effectful() {
                let deferred = DeferredValue::new(DeferredBody::NativeCall {
                    function: function_id,
                    args,
                });
                start_deferred_value(&deferred, yield_values)?;
                Ok(Value::Deferred(deferred))
            } else {
                fscript_std::execute_native_function(
                    function_id,
                    args,
                    |callee, args| call_value(callee, args, yield_values),
                    |value| force_value(value, yield_values),
                )
            }
        }
        std::cmp::Ordering::Greater => Err(RuntimeError::new(format!(
            "{} expected {} arguments but received {}",
            function.function.name(),
            function.function.arity(),
            all_args.len()
        ))
        .into()),
    }
}

fn force_task_input(value: Value, yield_values: YieldValues<'_>) -> RuntimeEval<Value> {
    match value {
        Value::Deferred(_) => force_value(value, yield_values),
        Value::Function(_) | Value::NativeFunction(_) => {
            let value = call_value(value, Vec::new(), yield_values)?;
            force_value(value, yield_values)
        }
        other => Err(RuntimeError::new(format!(
            "task inputs must be zero-argument callables or deferred tasks, found `{other}`"
        ))
        .into()),
    }
}

fn evaluate_binary_expr(
    operator: ir::BinaryOperator,
    left: Value,
    right: Value,
) -> RuntimeEval<Value> {
    match operator {
        ir::BinaryOperator::Add => match (left, right) {
            (Value::Number(left), Value::Number(right)) => Ok(Value::Number(left + right)),
            (Value::String(left), Value::String(right)) => Ok(Value::String(left + &right)),
            (Value::String(left), Value::Number(right)) => {
                Ok(Value::String(format!("{left}{right}")))
            }
            (Value::Number(left), Value::String(right)) => {
                Ok(Value::String(format!("{left}{right}")))
            }
            (left, right) => {
                Err(RuntimeError::new(format!("cannot add values `{left}` and `{right}`")).into())
            }
        },
        ir::BinaryOperator::Subtract => {
            compare_numeric_pair(left, right, |left, right| left - right, "subtract")
        }
        ir::BinaryOperator::Multiply => {
            compare_numeric_pair(left, right, |left, right| left * right, "multiply")
        }
        ir::BinaryOperator::Divide => {
            compare_numeric_pair(left, right, |left, right| left / right, "divide")
        }
        ir::BinaryOperator::Modulo => {
            compare_numeric_pair(left, right, |left, right| left % right, "apply `%` to")
        }
        ir::BinaryOperator::StrictEqual => Ok(Value::Boolean(
            left.structural_eq(&right).map_err(RuntimeControl::from)?,
        )),
        ir::BinaryOperator::StrictNotEqual => Ok(Value::Boolean(
            !left.structural_eq(&right).map_err(RuntimeControl::from)?,
        )),
        ir::BinaryOperator::Less => compare_numbers(left, right, |left, right| left < right),
        ir::BinaryOperator::LessEqual => compare_numbers(left, right, |left, right| left <= right),
        ir::BinaryOperator::Greater => compare_numbers(left, right, |left, right| left > right),
        ir::BinaryOperator::GreaterEqual => {
            compare_numbers(left, right, |left, right| left >= right)
        }
        ir::BinaryOperator::LogicalOr => compare_booleans(left, right, |left, right| left || right),
        ir::BinaryOperator::LogicalAnd => {
            compare_booleans(left, right, |left, right| left && right)
        }
        ir::BinaryOperator::NullishCoalesce => Ok(match left {
            Value::Null | Value::Undefined => right,
            value => value,
        }),
    }
}

fn compare_numeric_pair(
    left: Value,
    right: Value,
    combine: impl FnOnce(f64, f64) -> f64,
    verb: &str,
) -> RuntimeEval<Value> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => Ok(Value::Number(combine(left, right))),
        (left, right) => {
            Err(RuntimeError::new(format!("cannot {verb} values `{left}` and `{right}`")).into())
        }
    }
}

fn compare_numbers(
    left: Value,
    right: Value,
    compare: impl FnOnce(f64, f64) -> bool,
) -> RuntimeEval<Value> {
    match (left, right) {
        (Value::Number(left), Value::Number(right)) => Ok(Value::Boolean(compare(left, right))),
        (left, right) => Err(RuntimeError::new(format!(
            "expected numbers but found `{left}` and `{right}`"
        ))
        .into()),
    }
}

fn compare_booleans(
    left: Value,
    right: Value,
    combine: impl FnOnce(bool, bool) -> bool,
) -> RuntimeEval<Value> {
    match (left, right) {
        (Value::Boolean(left), Value::Boolean(right)) => Ok(Value::Boolean(combine(left, right))),
        (left, right) => Err(RuntimeError::new(format!(
            "expected booleans but found `{left}` and `{right}`"
        ))
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use std::collections::BTreeMap;

    use super::{run_module, run_program};
    use fscript_ir as ir;
    use fscript_runtime::Value;
    use fscript_source::Span;

    fn span() -> Span {
        Span::new(0, 0)
    }

    fn identifier_pattern(name: &str) -> ir::Pattern {
        ir::Pattern::Identifier {
            name: name.to_owned(),
            span: span(),
        }
    }

    fn binding(name: &str, value: ir::Expr) -> ir::ModuleItem {
        ir::ModuleItem::Binding(ir::BindingDecl {
            pattern: identifier_pattern(name),
            value,
            is_exported: false,
            span: span(),
        })
    }

    #[test]
    fn runs_curried_array_pipeline() {
        let module = ir::Module {
            items: vec![
                ir::ModuleItem::Import(ir::ImportDecl {
                    clause: ir::ImportClause::Default("Array".to_owned()),
                    source: "std:array".to_owned(),
                    source_span: span(),
                    span: span(),
                }),
                ir::ModuleItem::Binding(ir::BindingDecl {
                    pattern: ir::Pattern::Identifier {
                        name: "increment".to_owned(),
                        span: span(),
                    },
                    value: ir::Expr::Function {
                        parameters: vec![ir::Parameter {
                            pattern: ir::Pattern::Identifier {
                                name: "value".to_owned(),
                                span: span(),
                            },
                            span: span(),
                        }],
                        body: Box::new(ir::Expr::Binary {
                            operator: ir::BinaryOperator::Add,
                            left: Box::new(ir::Expr::Identifier {
                                name: "value".to_owned(),
                                span: span(),
                            }),
                            right: Box::new(ir::Expr::NumberLiteral {
                                value: 1.0,
                                span: span(),
                            }),
                            span: span(),
                        }),
                        is_generator: false,
                        span: span(),
                    },
                    is_exported: false,
                    span: span(),
                }),
                ir::ModuleItem::Binding(ir::BindingDecl {
                    pattern: ir::Pattern::Identifier {
                        name: "answer".to_owned(),
                        span: span(),
                    },
                    value: ir::Expr::Call {
                        callee: Box::new(ir::Expr::Member {
                            object: Box::new(ir::Expr::Identifier {
                                name: "Array".to_owned(),
                                span: span(),
                            }),
                            property: "map".to_owned(),
                            span: span(),
                        }),
                        args: vec![
                            ir::Expr::Identifier {
                                name: "increment".to_owned(),
                                span: span(),
                            },
                            ir::Expr::Array {
                                items: vec![
                                    ir::Expr::NumberLiteral {
                                        value: 1.0,
                                        span: span(),
                                    },
                                    ir::Expr::NumberLiteral {
                                        value: 2.0,
                                        span: span(),
                                    },
                                ],
                                span: span(),
                            },
                        ],
                        span: span(),
                    },
                    is_exported: false,
                    span: span(),
                }),
            ],
            exports: Vec::new(),
        };

        assert_eq!(
            run_module(&module),
            Ok(Some(Value::Array(vec![
                Value::Number(2.0),
                Value::Number(3.0)
            ])))
        );
    }

    #[test]
    fn forces_deferred_arguments_at_consumption_sites() {
        let module = ir::Module {
            items: vec![ir::ModuleItem::Binding(ir::BindingDecl {
                pattern: ir::Pattern::Identifier {
                    name: "answer".to_owned(),
                    span: span(),
                },
                value: ir::Expr::Call {
                    callee: Box::new(ir::Expr::Function {
                        parameters: vec![ir::Parameter {
                            pattern: ir::Pattern::Identifier {
                                name: "value".to_owned(),
                                span: span(),
                            },
                            span: span(),
                        }],
                        body: Box::new(ir::Expr::Binary {
                            operator: ir::BinaryOperator::Add,
                            left: Box::new(ir::Expr::Identifier {
                                name: "value".to_owned(),
                                span: span(),
                            }),
                            right: Box::new(ir::Expr::NumberLiteral {
                                value: 1.0,
                                span: span(),
                            }),
                            span: span(),
                        }),
                        is_generator: false,
                        span: span(),
                    }),
                    args: vec![ir::Expr::Unary {
                        operator: ir::UnaryOperator::Defer,
                        operand: Box::new(ir::Expr::NumberLiteral {
                            value: 41.0,
                            span: span(),
                        }),
                        span: span(),
                    }],
                    span: span(),
                },
                is_exported: false,
                span: span(),
            })],
            exports: Vec::new(),
        };

        assert_eq!(run_module(&module), Ok(Some(Value::Number(42.0))));
    }

    #[test]
    fn executes_effectful_native_calls_through_the_scheduler() {
        let path = std::env::temp_dir().join(format!(
            "fscript-interpreter-scheduler-{}.txt",
            std::process::id()
        ));
        fs::write(&path, "hello from runtime").expect("temp file should be writable");

        let module = ir::Module {
            items: vec![
                ir::ModuleItem::Import(ir::ImportDecl {
                    clause: ir::ImportClause::Default("FileSystem".to_owned()),
                    source: "std:filesystem".to_owned(),
                    source_span: span(),
                    span: span(),
                }),
                ir::ModuleItem::Binding(ir::BindingDecl {
                    pattern: ir::Pattern::Identifier {
                        name: "answer".to_owned(),
                        span: span(),
                    },
                    value: ir::Expr::Call {
                        callee: Box::new(ir::Expr::Member {
                            object: Box::new(ir::Expr::Identifier {
                                name: "FileSystem".to_owned(),
                                span: span(),
                            }),
                            property: "readFile".to_owned(),
                            span: span(),
                        }),
                        args: vec![ir::Expr::StringLiteral {
                            value: path.to_string_lossy().into_owned(),
                            span: span(),
                        }],
                        span: span(),
                    },
                    is_exported: false,
                    span: span(),
                }),
            ],
            exports: Vec::new(),
        };

        assert_eq!(
            run_module(&module),
            Ok(Some(Value::String("hello from runtime".to_owned())))
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn starts_effectful_native_calls_when_they_are_reached() {
        let path = std::env::temp_dir().join(format!(
            "fscript-interpreter-eager-start-{}.txt",
            std::process::id()
        ));
        let _ = fs::remove_file(&path);

        let module = ir::Module {
            items: vec![
                ir::ModuleItem::Import(ir::ImportDecl {
                    clause: ir::ImportClause::Default("FileSystem".to_owned()),
                    source: "std:filesystem".to_owned(),
                    source_span: span(),
                    span: span(),
                }),
                ir::ModuleItem::Binding(ir::BindingDecl {
                    pattern: ir::Pattern::Identifier {
                        name: "write".to_owned(),
                        span: span(),
                    },
                    value: ir::Expr::Call {
                        callee: Box::new(ir::Expr::Member {
                            object: Box::new(ir::Expr::Identifier {
                                name: "FileSystem".to_owned(),
                                span: span(),
                            }),
                            property: "writeFile".to_owned(),
                            span: span(),
                        }),
                        args: vec![
                            ir::Expr::StringLiteral {
                                value: path.to_string_lossy().into_owned(),
                                span: span(),
                            },
                            ir::Expr::StringLiteral {
                                value: "created eagerly".to_owned(),
                                span: span(),
                            },
                        ],
                        span: span(),
                    },
                    is_exported: false,
                    span: span(),
                }),
                ir::ModuleItem::Binding(ir::BindingDecl {
                    pattern: ir::Pattern::Identifier {
                        name: "answer".to_owned(),
                        span: span(),
                    },
                    value: ir::Expr::Call {
                        callee: Box::new(ir::Expr::Member {
                            object: Box::new(ir::Expr::Identifier {
                                name: "FileSystem".to_owned(),
                                span: span(),
                            }),
                            property: "exists".to_owned(),
                            span: span(),
                        }),
                        args: vec![ir::Expr::StringLiteral {
                            value: path.to_string_lossy().into_owned(),
                            span: span(),
                        }],
                        span: span(),
                    },
                    is_exported: false,
                    span: span(),
                }),
            ],
            exports: Vec::new(),
        };

        assert_eq!(run_module(&module), Ok(Some(Value::Boolean(true))));

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn run_program_reports_missing_entry_modules() {
        let error =
            run_program(&BTreeMap::new(), "missing").expect_err("missing modules should fail");

        assert!(
            error
                .message()
                .contains("runtime module `missing` is not available")
        );
    }

    #[test]
    fn run_program_rejects_circular_runtime_modules() {
        let modules = BTreeMap::from([
            (
                "a.fs".to_owned(),
                ir::Module {
                    items: vec![ir::ModuleItem::Import(ir::ImportDecl {
                        clause: ir::ImportClause::Default("b".to_owned()),
                        source: "b.fs".to_owned(),
                        source_span: span(),
                        span: span(),
                    })],
                    exports: vec![],
                },
            ),
            (
                "b.fs".to_owned(),
                ir::Module {
                    items: vec![ir::ModuleItem::Import(ir::ImportDecl {
                        clause: ir::ImportClause::Default("a".to_owned()),
                        source: "a.fs".to_owned(),
                        source_span: span(),
                        span: span(),
                    })],
                    exports: vec![],
                },
            ),
        ]);

        let error = run_program(&modules, "a.fs").expect_err("cycles should fail");

        assert!(
            error
                .message()
                .contains("circular runtime module dependency involving `a.fs`")
        );
    }

    #[test]
    fn run_program_reports_missing_user_exports_for_named_imports() {
        let modules = BTreeMap::from([
            (
                "entry.fs".to_owned(),
                ir::Module {
                    items: vec![ir::ModuleItem::Import(ir::ImportDecl {
                        clause: ir::ImportClause::Named(vec!["missing".to_owned()]),
                        source: "dep.fs".to_owned(),
                        source_span: span(),
                        span: span(),
                    })],
                    exports: vec![],
                },
            ),
            (
                "dep.fs".to_owned(),
                ir::Module {
                    items: vec![binding(
                        "value",
                        ir::Expr::NumberLiteral {
                            value: 1.0,
                            span: span(),
                        },
                    )],
                    exports: vec!["value".to_owned()],
                },
            ),
        ]);

        let error =
            run_program(&modules, "entry.fs").expect_err("missing named exports should fail");

        assert!(
            error
                .message()
                .contains("module `dep.fs` does not export `missing`")
        );
    }

    #[test]
    fn run_module_rejects_duplicate_bindings_from_patterns() {
        let module = ir::Module {
            items: vec![ir::ModuleItem::Binding(ir::BindingDecl {
                pattern: ir::Pattern::Array {
                    items: vec![identifier_pattern("value"), identifier_pattern("value")],
                    span: span(),
                },
                value: ir::Expr::Array {
                    items: vec![
                        ir::Expr::NumberLiteral {
                            value: 1.0,
                            span: span(),
                        },
                        ir::Expr::NumberLiteral {
                            value: 2.0,
                            span: span(),
                        },
                    ],
                    span: span(),
                },
                is_exported: false,
                span: span(),
            })],
            exports: vec![],
        };

        let error = run_module(&module).expect_err("duplicate pattern bindings should fail");

        assert!(
            error
                .message()
                .contains("binding `value` is already defined in this scope")
        );
    }

    #[test]
    fn run_module_reports_binding_pattern_mismatches() {
        let module = ir::Module {
            items: vec![ir::ModuleItem::Binding(ir::BindingDecl {
                pattern: ir::Pattern::Literal(ir::LiteralPattern::String {
                    value: "expected".to_owned(),
                    span: span(),
                }),
                value: ir::Expr::StringLiteral {
                    value: "actual".to_owned(),
                    span: span(),
                },
                is_exported: false,
                span: span(),
            })],
            exports: vec![],
        };

        let error = run_module(&module).expect_err("literal pattern mismatches should fail");

        assert!(error.message().contains("does not match binding pattern"));
    }

    #[test]
    fn run_program_reports_missing_declared_exports() {
        let modules = BTreeMap::from([(
            "entry.fs".to_owned(),
            ir::Module {
                items: vec![binding(
                    "value",
                    ir::Expr::NumberLiteral {
                        value: 1.0,
                        span: span(),
                    },
                )],
                exports: vec!["missing".to_owned()],
            },
        )]);

        let error = run_program(&modules, "entry.fs").expect_err("missing exports should fail");

        assert!(
            error
                .message()
                .contains("module export `missing` is not defined")
        );
    }
}
