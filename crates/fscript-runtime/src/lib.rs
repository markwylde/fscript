//! Shared runtime value model for the first executable slice.

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};

use fscript_ir as ir;
use thiserror::Error;

/// Runtime errors surfaced from the shared execution layer.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{message}")]
pub struct RuntimeError {
    message: String,
}

impl RuntimeError {
    /// Creates a new runtime error.
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Returns the runtime error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Runtime environment used by closures and deferred values.
pub type Environment = BTreeMap<String, Value>;

/// Shared runtime values for interpreter and future codegen.
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
    /// Checks structural equality for plain comparable values.
    pub fn structural_eq(&self, other: &Self) -> Result<bool, RuntimeError> {
        match (self, other) {
            (Self::Function(_), _)
            | (_, Self::Function(_))
            | (Self::NativeFunction(_), _)
            | (_, Self::NativeFunction(_))
            | (Self::Deferred(_), _)
            | (_, Self::Deferred(_)) => Err(RuntimeError::new(
                "functions and deferred values cannot be compared with `===` or `!==`",
            )),
            (Self::String(left), Self::String(right)) => Ok(left == right),
            (Self::Number(left), Self::Number(right)) => Ok(left == right),
            (Self::Boolean(left), Self::Boolean(right)) => Ok(left == right),
            (Self::Null, Self::Null) | (Self::Undefined, Self::Undefined) => Ok(true),
            (Self::Array(left), Self::Array(right))
            | (Self::Sequence(left), Self::Sequence(right))
            | (Self::Array(left), Self::Sequence(right))
            | (Self::Sequence(left), Self::Array(right)) => {
                if left.len() != right.len() {
                    return Ok(false);
                }

                for (left, right) in left.iter().zip(right.iter()) {
                    if !left.structural_eq(right)? {
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            (Self::Record(left), Self::Record(right)) => {
                if left.len() != right.len() {
                    return Ok(false);
                }

                for (name, left_value) in left {
                    let Some(right_value) = right.get(name) else {
                        return Ok(false);
                    };

                    if !left_value.structural_eq(right_value)? {
                        return Ok(false);
                    }
                }

                Ok(true)
            }
            _ => Ok(false),
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
                for (index, (name, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        write!(f, ", ")?;
                    }
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
            Self::NativeFunction(function) => {
                write!(f, "[function {}]", function.function.name())
            }
        }
    }
}

fn write_nested_value(f: &mut std::fmt::Formatter<'_>, value: &Value) -> std::fmt::Result {
    match value {
        Value::String(string) => write!(f, "'{string}'"),
        other => write!(f, "{other}"),
    }
}

/// Deferred runtime value with memoized evaluation.
#[derive(Clone, Debug, PartialEq)]
pub struct DeferredValue {
    pub body: DeferredBody,
    state: Rc<RefCell<TaskState>>,
}

impl DeferredValue {
    /// Creates a new deferred runtime value in the `created` state.
    #[must_use]
    pub fn new(body: DeferredBody) -> Self {
        Self {
            body,
            state: Rc::new(RefCell::new(TaskState::created())),
        }
    }

    /// Returns a deferred runtime value that has already completed.
    #[must_use]
    pub fn from_outcome(body: DeferredBody, outcome: DeferredOutcome) -> Self {
        Self {
            body,
            state: Rc::new(RefCell::new(TaskState::finished(outcome))),
        }
    }

    /// Returns the current task status for this deferred value.
    #[must_use]
    pub fn status(&self) -> TaskStatus {
        self.state.borrow().status
    }

    /// Returns the memoized deferred outcome when it exists.
    #[must_use]
    pub fn outcome(&self) -> Option<DeferredOutcome> {
        self.state.borrow().outcome.clone()
    }

    /// Marks the deferred value as ready to run.
    pub fn mark_ready(&self) {
        let mut state = self.state.borrow_mut();
        if matches!(state.status, TaskStatus::Created) {
            state.status = TaskStatus::Ready;
        }
    }

    /// Marks the deferred value as currently running.
    pub fn mark_running(&self) {
        self.state.borrow_mut().status = TaskStatus::Running;
    }

    /// Marks the deferred value as waiting on a dependency.
    pub fn mark_waiting(&self) {
        self.state.borrow_mut().status = TaskStatus::Waiting;
    }

    /// Stores the completed outcome and marks the task as finished.
    pub fn finish(&self, outcome: DeferredOutcome) {
        let mut state = self.state.borrow_mut();
        state.status = TaskStatus::Completed;
        state.outcome = Some(outcome);
    }

    /// Marks the deferred value as failed without a completed outcome.
    pub fn fail(&self) {
        self.state.borrow_mut().status = TaskStatus::Failed;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskStatus {
    Created,
    Ready,
    Running,
    Waiting,
    Completed,
    Failed,
    Canceled,
}

#[derive(Clone, Debug, PartialEq)]
struct TaskState {
    status: TaskStatus,
    outcome: Option<DeferredOutcome>,
}

impl TaskState {
    fn created() -> Self {
        Self {
            status: TaskStatus::Created,
            outcome: None,
        }
    }

    fn finished(outcome: DeferredOutcome) -> Self {
        Self {
            status: TaskStatus::Completed,
            outcome: Some(outcome),
        }
    }
}

/// Deferred work captured by the runtime.
#[derive(Clone, Debug, PartialEq)]
pub enum DeferredBody {
    Expr {
        expr: Box<ir::Expr>,
        environment: Environment,
    },
    NativeCall {
        function: NativeFunction,
        args: Vec<Value>,
    },
    Call(Box<Value>),
    Batch(Vec<Value>),
    Race(Vec<Value>),
}

/// Deferred resolution state.
#[derive(Clone, Debug, PartialEq)]
pub enum DeferredOutcome {
    Value(Value),
    Throw(Value),
}

/// Host callbacks required by the runtime scheduler to execute deferred work.
pub trait SchedulerExecutor<E> {
    /// Evaluates a deferred expression body into a memoizable outcome.
    fn evaluate_expr_task(
        &mut self,
        expr: &ir::Expr,
        environment: &Environment,
    ) -> Result<DeferredOutcome, E>;

    /// Executes a native effectful task once the scheduler reaches it.
    fn execute_native_task(
        &mut self,
        function: NativeFunction,
        args: Vec<Value>,
    ) -> Result<Value, E>;

    /// Forces a zero-argument task input to its resulting value.
    fn force_task_input(&mut self, value: Value) -> Result<Value, E>;
}

/// Single-threaded task scheduler for deferred runtime work.
#[derive(Default)]
pub struct SingleThreadedScheduler {
    ready: VecDeque<DeferredValue>,
}

impl SingleThreadedScheduler {
    /// Creates an empty scheduler.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a deferred handle immediately, draining any ready work in source order.
    pub fn start_deferred<E, X>(
        &mut self,
        deferred: DeferredValue,
        executor: &mut X,
    ) -> Result<(), E>
    where
        E: From<RuntimeError>,
        X: SchedulerExecutor<E>,
    {
        if deferred.outcome().is_some() {
            return Ok(());
        }

        self.enqueue(deferred);
        self.drain(executor)
    }

    /// Forces a deferred handle to completion, running queued work in source order.
    pub fn force_deferred<E, X>(
        &mut self,
        deferred: DeferredValue,
        executor: &mut X,
    ) -> Result<DeferredOutcome, E>
    where
        E: From<RuntimeError>,
        X: SchedulerExecutor<E>,
    {
        if let Some(outcome) = deferred.outcome() {
            return Ok(outcome);
        }

        self.start_deferred(deferred.clone(), executor)?;
        Ok(deferred
            .outcome()
            .expect("draining a started deferred value must produce an outcome"))
    }

    fn enqueue(&mut self, deferred: DeferredValue) {
        if deferred.outcome().is_some() {
            return;
        }

        if matches!(deferred.status(), TaskStatus::Created) {
            deferred.mark_ready();
        }

        self.ready.push_back(deferred);
    }

    fn drain<E, X>(&mut self, executor: &mut X) -> Result<(), E>
    where
        E: From<RuntimeError>,
        X: SchedulerExecutor<E>,
    {
        while let Some(deferred) = self.ready.pop_front() {
            if deferred.outcome().is_some() {
                continue;
            }

            deferred.mark_running();

            let outcome = match &deferred.body {
                DeferredBody::Expr { expr, environment } => {
                    executor.evaluate_expr_task(expr, environment)
                }
                DeferredBody::NativeCall { function, args } => executor
                    .execute_native_task(*function, args.clone())
                    .map(DeferredOutcome::Value),
                DeferredBody::Call(callee) => executor
                    .force_task_input((**callee).clone())
                    .map(DeferredOutcome::Value),
                DeferredBody::Batch(tasks) => {
                    let mut values = Vec::with_capacity(tasks.len());
                    for task in tasks {
                        deferred.mark_waiting();
                        values.push(executor.force_task_input(task.clone())?);
                        deferred.mark_running();
                    }
                    Ok(DeferredOutcome::Value(Value::Array(values)))
                }
                DeferredBody::Race(tasks) => {
                    let Some(first) = tasks.first() else {
                        return Err(E::from(RuntimeError::new(
                            "Task.race expects a non-empty array of tasks",
                        )));
                    };

                    deferred.mark_waiting();
                    executor
                        .force_task_input(first.clone())
                        .map(DeferredOutcome::Value)
                }
            };

            match outcome {
                Ok(outcome) => deferred.finish(outcome),
                Err(error) => {
                    deferred.fail();
                    return Err(error);
                }
            }
        }

        Ok(())
    }
}

/// Curried user-defined function value.
#[derive(Clone, Debug, PartialEq)]
pub struct FunctionValue {
    pub parameters: Vec<ir::Parameter>,
    pub body: Box<ir::Expr>,
    pub environment: Environment,
    pub applied_args: Vec<Value>,
    pub is_generator: bool,
}

impl FunctionValue {
    /// Returns the total arity of the function.
    #[must_use]
    pub fn arity(&self) -> usize {
        self.parameters.len()
    }

    /// Returns a new partially applied function value.
    #[must_use]
    pub fn with_args(&self, args: Vec<Value>) -> Self {
        Self {
            parameters: self.parameters.clone(),
            body: self.body.clone(),
            environment: self.environment.clone(),
            applied_args: args,
            is_generator: self.is_generator,
        }
    }
}

/// Host-native function value with curried argument capture.
#[derive(Clone, Debug, PartialEq)]
pub struct NativeFunctionValue {
    pub function: NativeFunction,
    pub applied_args: Vec<Value>,
}

impl NativeFunctionValue {
    /// Creates a new native function.
    #[must_use]
    pub fn new(function: NativeFunction) -> Self {
        Self {
            function,
            applied_args: Vec::new(),
        }
    }

    /// Returns a new partially applied native function value.
    #[must_use]
    pub fn with_args(&self, args: Vec<Value>) -> Self {
        Self {
            function: self.function,
            applied_args: args,
        }
    }
}

/// Host-native functions supported by the first stdlib slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFunction {
    ObjectSpread,
    ArrayMap,
    ArrayFilter,
    ArrayLength,
    HttpServe,
    JsonParse,
    JsonStringify,
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
    /// Returns the human-readable function name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ObjectSpread => "Object.spread",
            Self::ArrayMap => "Array.map",
            Self::ArrayFilter => "Array.filter",
            Self::ArrayLength => "Array.length",
            Self::HttpServe => "Http.serve",
            Self::JsonParse => "Json.parse",
            Self::JsonStringify => "Json.stringify",
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

    /// Returns the expected arity for currying support.
    #[must_use]
    pub const fn arity(self) -> usize {
        match self {
            Self::ObjectSpread => 2,
            Self::ArrayMap | Self::ArrayFilter => 2,
            Self::ArrayLength => 1,
            Self::HttpServe => 2,
            Self::JsonParse | Self::JsonStringify => 1,
            Self::FilesystemReadFile
            | Self::FilesystemExists
            | Self::FilesystemDeleteFile
            | Self::FilesystemReadDir => 1,
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
            | Self::TaskDefer
            | Self::TaskForce => 1,
            Self::TaskAll | Self::TaskRace | Self::TaskSpawn => 1,
            Self::ResultWithDefault => 2,
        }
    }

    /// Returns whether the interpreter should force arguments before dispatch.
    #[must_use]
    pub const fn forces_arguments(self) -> bool {
        !matches!(
            self,
            Self::TaskAll | Self::TaskRace | Self::TaskSpawn | Self::TaskDefer | Self::TaskForce
        )
    }

    /// Returns whether the native function performs eager host effects.
    #[must_use]
    pub const fn is_effectful(self) -> bool {
        matches!(
            self,
            Self::HttpServe
                | Self::FilesystemReadFile
                | Self::FilesystemWriteFile
                | Self::FilesystemExists
                | Self::FilesystemDeleteFile
                | Self::FilesystemReadDir
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DeferredBody, DeferredOutcome, DeferredValue, Environment, FunctionValue, NativeFunction,
        NativeFunctionValue, RuntimeError, SchedulerExecutor, SingleThreadedScheduler, Value,
    };
    use fscript_ir::{Expr, Parameter, Pattern};
    use fscript_source::Span;
    use std::collections::BTreeMap;

    #[test]
    fn display_formats_nested_values() {
        let value = Value::Record(BTreeMap::from([
            ("name".to_owned(), Value::String("Ada".to_owned())),
            (
                "scores".to_owned(),
                Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]),
            ),
        ]));

        assert_eq!(value.to_string(), "{ name: 'Ada', scores: [1, 2] }");
    }

    #[test]
    fn structural_equality_compares_plain_data() {
        let left = Value::Record(BTreeMap::from([
            ("ok".to_owned(), Value::Boolean(true)),
            (
                "values".to_owned(),
                Value::Array(vec![Value::Number(1.0), Value::Null]),
            ),
        ]));
        let right = Value::Record(BTreeMap::from([
            ("ok".to_owned(), Value::Boolean(true)),
            (
                "values".to_owned(),
                Value::Array(vec![Value::Number(1.0), Value::Null]),
            ),
        ]));

        assert_eq!(left.structural_eq(&right), Ok(true));
    }

    #[test]
    fn structural_equality_rejects_functions() {
        let function = Value::Function(FunctionValue {
            parameters: vec![Parameter {
                pattern: Pattern::Identifier {
                    name: "value".to_owned(),
                    span: Span::new(0, 5),
                },
                span: Span::new(0, 5),
            }],
            body: Box::new(Expr::Identifier {
                name: "value".to_owned(),
                span: Span::new(0, 5),
            }),
            environment: Environment::new(),
            applied_args: Vec::new(),
            is_generator: false,
        });

        assert_eq!(
            function.structural_eq(&Value::Null),
            Err(RuntimeError::new(
                "functions and deferred values cannot be compared with `===` or `!==`"
            ))
        );
    }

    #[test]
    fn value_display_and_structural_equality_cover_scalar_and_mismatch_cases() {
        let generator = Value::Function(FunctionValue {
            parameters: vec![],
            body: Box::new(Expr::Null {
                span: Span::new(0, 0),
            }),
            environment: Environment::new(),
            applied_args: Vec::new(),
            is_generator: true,
        });
        let function = Value::Function(FunctionValue {
            parameters: vec![],
            body: Box::new(Expr::Null {
                span: Span::new(0, 0),
            }),
            environment: Environment::new(),
            applied_args: Vec::new(),
            is_generator: false,
        });
        let native = Value::NativeFunction(NativeFunctionValue::new(NativeFunction::ArrayLength));

        assert_eq!(
            Value::String("a".to_owned()).structural_eq(&Value::String("a".to_owned())),
            Ok(true)
        );
        assert_eq!(
            Value::Sequence(vec![Value::Number(1.0)])
                .structural_eq(&Value::Array(vec![Value::Number(1.0)])),
            Ok(true)
        );
        assert_eq!(
            Value::Sequence(vec![Value::Number(1.0)])
                .structural_eq(&Value::Sequence(vec![Value::Number(1.0)])),
            Ok(true)
        );
        assert_eq!(
            Value::Array(vec![Value::Number(1.0)])
                .structural_eq(&Value::Sequence(vec![Value::Number(1.0)])),
            Ok(true)
        );
        assert_eq!(
            Value::Array(vec![Value::Number(1.0)])
                .structural_eq(&Value::Array(vec![Value::Number(1.0), Value::Number(2.0)])),
            Ok(false)
        );
        assert_eq!(
            Value::Array(vec![Value::Number(1.0), Value::Number(2.0)])
                .structural_eq(&Value::Array(vec![Value::Number(1.0), Value::Number(3.0)])),
            Ok(false)
        );
        assert_eq!(
            Value::Record(BTreeMap::from([(
                "name".to_owned(),
                Value::String("Ada".to_owned())
            )]))
            .structural_eq(&Value::Record(BTreeMap::new())),
            Ok(false)
        );
        assert_eq!(
            Value::Record(BTreeMap::from([(
                "name".to_owned(),
                Value::String("Ada".to_owned())
            )]))
            .structural_eq(&Value::Record(BTreeMap::from([(
                "age".to_owned(),
                Value::Number(1.0)
            )]))),
            Ok(false)
        );
        assert_eq!(
            Value::Record(BTreeMap::from([(
                "user".to_owned(),
                Value::Record(BTreeMap::from([(
                    "name".to_owned(),
                    Value::String("Ada".to_owned())
                )]))
            )]))
            .structural_eq(&Value::Record(BTreeMap::from([(
                "user".to_owned(),
                Value::Record(BTreeMap::from([(
                    "name".to_owned(),
                    Value::String("Grace".to_owned())
                )]))
            )]))),
            Ok(false)
        );
        assert_eq!(Value::Null.structural_eq(&Value::Undefined), Ok(false));

        assert_eq!(Value::String("Ada".to_owned()).to_string(), "Ada");
        assert_eq!(Value::Boolean(true).to_string(), "true");
        assert_eq!(Value::Null.to_string(), "Null");
        assert_eq!(Value::Undefined.to_string(), "Undefined");
        assert_eq!(
            Value::Sequence(vec![Value::String("Ada".to_owned()), Value::Boolean(true)])
                .to_string(),
            "Sequence(['Ada', true])"
        );
        assert_eq!(generator.to_string(), "[generator]");
        assert_eq!(function.to_string(), "[function]");
        assert_eq!(native.to_string(), "[function Array.length]");
    }

    #[test]
    fn native_function_values_remember_partial_args() {
        let native = NativeFunctionValue::new(NativeFunction::ArrayLength)
            .with_args(vec![Value::Array(vec![Value::Number(1.0)])]);

        assert_eq!(native.function.arity(), 1);
        assert_eq!(native.applied_args.len(), 1);
    }

    #[test]
    fn function_values_preserve_arity_and_partial_application() {
        let function = FunctionValue {
            parameters: vec![
                Parameter {
                    pattern: Pattern::Identifier {
                        name: "left".to_owned(),
                        span: Span::new(0, 4),
                    },
                    span: Span::new(0, 4),
                },
                Parameter {
                    pattern: Pattern::Identifier {
                        name: "right".to_owned(),
                        span: Span::new(5, 10),
                    },
                    span: Span::new(5, 10),
                },
            ],
            body: Box::new(Expr::Identifier {
                name: "left".to_owned(),
                span: Span::new(0, 4),
            }),
            environment: BTreeMap::from([("captured".to_owned(), Value::Number(10.0))]),
            applied_args: vec![Value::Number(1.0)],
            is_generator: true,
        };

        let partially_applied = function.with_args(vec![Value::Number(2.0)]);

        assert_eq!(function.arity(), 2);
        assert_eq!(partially_applied.parameters, function.parameters);
        assert_eq!(partially_applied.body, function.body);
        assert_eq!(partially_applied.environment, function.environment);
        assert_eq!(partially_applied.is_generator, function.is_generator);
        assert_eq!(partially_applied.applied_args, vec![Value::Number(2.0)]);
    }

    #[test]
    fn deferred_values_can_hold_memoized_outcomes() {
        let deferred = Value::Deferred(super::DeferredValue::from_outcome(
            DeferredBody::Expr {
                expr: Box::new(Expr::Null {
                    span: Span::new(0, 0),
                }),
                environment: Environment::new(),
            },
            DeferredOutcome::Value(Value::Null),
        ));

        assert_eq!(deferred.to_string(), "[deferred]");
    }

    #[test]
    fn runtime_error_message_and_mark_ready_idempotence() {
        let deferred = super::DeferredValue::new(DeferredBody::Call(Box::new(Value::Null)));
        let error = RuntimeError::new("runtime message");

        assert_eq!(error.message(), "runtime message");
        assert_eq!(deferred.status(), super::TaskStatus::Created);

        deferred.mark_ready();
        assert_eq!(deferred.status(), super::TaskStatus::Ready);

        deferred.mark_running();
        deferred.mark_ready();
        assert_eq!(deferred.status(), super::TaskStatus::Running);
    }

    #[test]
    fn deferred_values_track_task_state_transitions() {
        let deferred = super::DeferredValue::new(DeferredBody::Call(Box::new(Value::Null)));
        assert_eq!(deferred.status(), super::TaskStatus::Created);

        deferred.mark_ready();
        assert_eq!(deferred.status(), super::TaskStatus::Ready);

        deferred.mark_running();
        assert_eq!(deferred.status(), super::TaskStatus::Running);

        deferred.mark_waiting();
        assert_eq!(deferred.status(), super::TaskStatus::Waiting);

        deferred.finish(DeferredOutcome::Value(Value::Number(1.0)));
        assert_eq!(deferred.status(), super::TaskStatus::Completed);
        assert_eq!(
            deferred.outcome(),
            Some(DeferredOutcome::Value(Value::Number(1.0)))
        );
    }

    #[test]
    fn native_function_metadata_covers_every_variant() {
        let cases = [
            (
                NativeFunction::ObjectSpread,
                "Object.spread",
                2,
                true,
                false,
            ),
            (NativeFunction::ArrayMap, "Array.map", 2, true, false),
            (NativeFunction::ArrayFilter, "Array.filter", 2, true, false),
            (NativeFunction::ArrayLength, "Array.length", 1, true, false),
            (NativeFunction::HttpServe, "Http.serve", 2, true, true),
            (NativeFunction::JsonParse, "Json.parse", 1, true, false),
            (
                NativeFunction::JsonStringify,
                "Json.stringify",
                1,
                true,
                false,
            ),
            (
                NativeFunction::FilesystemReadFile,
                "FileSystem.readFile",
                1,
                true,
                true,
            ),
            (
                NativeFunction::FilesystemWriteFile,
                "FileSystem.writeFile",
                2,
                true,
                true,
            ),
            (
                NativeFunction::FilesystemExists,
                "FileSystem.exists",
                1,
                true,
                true,
            ),
            (
                NativeFunction::FilesystemDeleteFile,
                "FileSystem.deleteFile",
                1,
                true,
                true,
            ),
            (
                NativeFunction::FilesystemReadDir,
                "FileSystem.readDir",
                1,
                true,
                true,
            ),
            (NativeFunction::StringTrim, "String.trim", 1, true, false),
            (
                NativeFunction::StringUppercase,
                "String.uppercase",
                1,
                true,
                false,
            ),
            (
                NativeFunction::StringLowercase,
                "String.lowercase",
                1,
                true,
                false,
            ),
            (
                NativeFunction::StringIsDigits,
                "String.isDigits",
                1,
                true,
                false,
            ),
            (NativeFunction::NumberParse, "Number.parse", 1, true, false),
            (NativeFunction::ResultOk, "Result.ok", 1, true, false),
            (NativeFunction::ResultError, "Result.error", 1, true, false),
            (NativeFunction::ResultIsOk, "Result.isOk", 1, true, false),
            (
                NativeFunction::ResultIsError,
                "Result.isError",
                1,
                true,
                false,
            ),
            (
                NativeFunction::ResultWithDefault,
                "Result.withDefault",
                2,
                true,
                false,
            ),
            (NativeFunction::TaskAll, "Task.all", 1, false, false),
            (NativeFunction::TaskRace, "Task.race", 1, false, false),
            (NativeFunction::TaskSpawn, "Task.spawn", 1, false, false),
            (NativeFunction::TaskDefer, "Task.defer", 1, false, false),
            (NativeFunction::TaskForce, "Task.force", 1, false, false),
        ];

        for (function, name, arity, forces_arguments, is_effectful) in cases {
            assert_eq!(function.name(), name);
            assert_eq!(function.arity(), arity);
            assert_eq!(function.forces_arguments(), forces_arguments);
            assert_eq!(function.is_effectful(), is_effectful);
        }
    }

    struct TestExecutor;

    impl SchedulerExecutor<RuntimeError> for TestExecutor {
        fn evaluate_expr_task(
            &mut self,
            expr: &Expr,
            _environment: &Environment,
        ) -> Result<DeferredOutcome, RuntimeError> {
            match expr {
                Expr::NumberLiteral { value, .. } => {
                    Ok(DeferredOutcome::Value(Value::Number(*value)))
                }
                other => Err(RuntimeError::new(format!(
                    "test executor cannot evaluate `{other:?}`"
                ))),
            }
        }

        fn execute_native_task(
            &mut self,
            function: NativeFunction,
            args: Vec<Value>,
        ) -> Result<Value, RuntimeError> {
            match function {
                NativeFunction::FilesystemReadFile => {
                    let [value]: [Value; 1] = args.try_into().map_err(|_| {
                        RuntimeError::new("test executor expected exactly 1 native argument")
                    })?;
                    Ok(value)
                }
                other => Err(RuntimeError::new(format!(
                    "test executor cannot execute native task `{}`",
                    other.name()
                ))),
            }
        }

        fn force_task_input(&mut self, value: Value) -> Result<Value, RuntimeError> {
            match value {
                Value::Deferred(deferred) => {
                    let mut scheduler = SingleThreadedScheduler::new();
                    match scheduler.force_deferred(deferred, self)? {
                        DeferredOutcome::Value(value) => Ok(value),
                        DeferredOutcome::Throw(value) => Err(RuntimeError::new(format!(
                            "uncaught thrown value `{value}`"
                        ))),
                    }
                }
                other => Ok(other),
            }
        }
    }

    #[test]
    fn scheduler_forces_deferred_expression_bodies() {
        let deferred = DeferredValue::new(DeferredBody::Expr {
            expr: Box::new(Expr::NumberLiteral {
                value: 42.0,
                span: Span::new(0, 0),
            }),
            environment: Environment::new(),
        });
        let mut scheduler = SingleThreadedScheduler::new();
        let outcome = scheduler
            .force_deferred(deferred.clone(), &mut TestExecutor)
            .expect("scheduler should force expression bodies");

        assert_eq!(outcome, DeferredOutcome::Value(Value::Number(42.0)));
        assert_eq!(deferred.status(), super::TaskStatus::Completed);
        assert_eq!(deferred.outcome(), Some(outcome));
    }

    #[test]
    fn scheduler_can_start_deferred_work_without_forcing_the_handle() {
        let deferred = DeferredValue::new(DeferredBody::Expr {
            expr: Box::new(Expr::NumberLiteral {
                value: 7.0,
                span: Span::new(0, 0),
            }),
            environment: Environment::new(),
        });
        let mut scheduler = SingleThreadedScheduler::new();

        scheduler
            .start_deferred(deferred.clone(), &mut TestExecutor)
            .expect("scheduler should start expression bodies eagerly");

        assert_eq!(deferred.status(), super::TaskStatus::Completed);
        assert_eq!(
            deferred.outcome(),
            Some(DeferredOutcome::Value(Value::Number(7.0)))
        );
    }

    #[test]
    fn scheduler_short_circuit_and_failure_paths_are_reported() {
        let completed = DeferredValue::from_outcome(
            DeferredBody::Call(Box::new(Value::Null)),
            DeferredOutcome::Value(Value::Number(9.0)),
        );
        let mut scheduler = SingleThreadedScheduler::new();

        scheduler
            .start_deferred(completed.clone(), &mut TestExecutor)
            .expect("completed deferred values should short circuit");
        assert_eq!(
            scheduler
                .force_deferred(completed.clone(), &mut TestExecutor)
                .expect("completed deferred values should return memoized outcomes"),
            DeferredOutcome::Value(Value::Number(9.0))
        );

        let expression_failure = DeferredValue::new(DeferredBody::Expr {
            expr: Box::new(Expr::Null {
                span: Span::new(0, 0),
            }),
            environment: Environment::new(),
        });
        let error = scheduler
            .force_deferred(expression_failure.clone(), &mut TestExecutor)
            .expect_err("unsupported expr bodies should fail");
        assert!(error.message().contains("test executor cannot evaluate"));
        assert_eq!(expression_failure.status(), super::TaskStatus::Failed);

        let native_failure = DeferredValue::new(DeferredBody::NativeCall {
            function: NativeFunction::HttpServe,
            args: vec![Value::Null, Value::Null],
        });
        let error = scheduler
            .force_deferred(native_failure.clone(), &mut TestExecutor)
            .expect_err("unsupported native tasks should fail");
        assert!(
            error
                .message()
                .contains("test executor cannot execute native task")
        );
        assert_eq!(native_failure.status(), super::TaskStatus::Failed);

        assert_eq!(
            TestExecutor
                .force_task_input(Value::Number(3.0))
                .expect("plain values should pass through"),
            Value::Number(3.0)
        );

        let thrown = DeferredValue::from_outcome(
            DeferredBody::Call(Box::new(Value::Null)),
            DeferredOutcome::Throw(Value::String("boom".to_owned())),
        );
        let error = TestExecutor
            .force_task_input(Value::Deferred(thrown))
            .expect_err("thrown deferred values should become runtime errors");
        assert!(error.message().contains("uncaught thrown value `boom`"));
    }

    #[test]
    fn scheduler_skips_completed_work_already_in_the_queue() {
        let completed = DeferredValue::from_outcome(
            DeferredBody::Call(Box::new(Value::Null)),
            DeferredOutcome::Value(Value::Number(1.0)),
        );
        let mut scheduler = SingleThreadedScheduler {
            ready: std::collections::VecDeque::from([completed]),
        };

        scheduler
            .drain(&mut TestExecutor)
            .expect("completed queued work should be skipped");
        assert!(scheduler.ready.is_empty());
    }

    #[test]
    fn enqueue_ignores_completed_deferred_values() {
        let completed = DeferredValue::from_outcome(
            DeferredBody::Call(Box::new(Value::Null)),
            DeferredOutcome::Value(Value::Number(1.0)),
        );
        let mut scheduler = SingleThreadedScheduler::new();

        scheduler.enqueue(completed);

        assert!(scheduler.ready.is_empty());
    }

    #[test]
    fn scheduler_forces_batched_tasks_in_order() {
        let first = DeferredValue::from_outcome(
            DeferredBody::Call(Box::new(Value::Null)),
            DeferredOutcome::Value(Value::Number(1.0)),
        );
        let second = DeferredValue::from_outcome(
            DeferredBody::Call(Box::new(Value::Null)),
            DeferredOutcome::Value(Value::Number(2.0)),
        );
        let batch = DeferredValue::new(DeferredBody::Batch(vec![
            Value::Deferred(first),
            Value::Deferred(second),
        ]));
        let mut scheduler = SingleThreadedScheduler::new();
        let outcome = scheduler
            .force_deferred(batch, &mut TestExecutor)
            .expect("scheduler should force task batches");

        assert_eq!(
            outcome,
            DeferredOutcome::Value(Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]))
        );
    }

    #[test]
    fn scheduler_supports_call_and_race_tasks() {
        let call = DeferredValue::new(DeferredBody::Call(Box::new(Value::Number(3.0))));
        let mut scheduler = SingleThreadedScheduler::new();
        let outcome = scheduler
            .force_deferred(call, &mut TestExecutor)
            .expect("call tasks should force the callee value");
        assert_eq!(outcome, DeferredOutcome::Value(Value::Number(3.0)));

        let race = DeferredValue::new(DeferredBody::Race(vec![
            Value::Number(9.0),
            Value::Number(10.0),
        ]));
        let outcome = scheduler
            .force_deferred(race, &mut TestExecutor)
            .expect("race should return the first task value");
        assert_eq!(outcome, DeferredOutcome::Value(Value::Number(9.0)));

        let empty_race = DeferredValue::new(DeferredBody::Race(vec![]));
        let error = scheduler
            .force_deferred(empty_race, &mut TestExecutor)
            .expect_err("empty races should fail");
        assert_eq!(
            error.message(),
            "Task.race expects a non-empty array of tasks"
        );
    }

    #[test]
    fn scheduler_executes_native_call_tasks() {
        let deferred = DeferredValue::new(DeferredBody::NativeCall {
            function: NativeFunction::FilesystemReadFile,
            args: vec![Value::String("native result".to_owned())],
        });
        let mut scheduler = SingleThreadedScheduler::new();
        let outcome = scheduler
            .force_deferred(deferred.clone(), &mut TestExecutor)
            .expect("scheduler should execute native call tasks");

        assert_eq!(
            outcome,
            DeferredOutcome::Value(Value::String("native result".to_owned()))
        );
        assert_eq!(deferred.status(), super::TaskStatus::Completed);
        assert_eq!(deferred.outcome(), Some(outcome));
    }

    #[test]
    fn test_executor_reports_invalid_native_argument_shapes() {
        let error = TestExecutor
            .execute_native_task(
                NativeFunction::FilesystemReadFile,
                vec![Value::Null, Value::Null],
            )
            .expect_err("wrong native arity should fail");

        assert!(
            error
                .message()
                .contains("test executor expected exactly 1 native argument")
        );
    }
}
