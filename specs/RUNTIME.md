# FScript Runtime Specification

Status: Draft 0.1

## 1. Goal

Define the native runtime model for FScript.

The runtime exists to execute well-typed FScript programs efficiently and predictably without relying on a JavaScript engine.

Draft 0.1 assumes:

- implementation in Rust
- single-threaded scheduling
- native standard-library capabilities
- ahead-of-time compilation support
- interpreter support during early milestones and for testing

## 2. Design Principles

The runtime should be:

- native
- minimal
- predictable
- observable
- easy to reason about
- efficient for pure code

Most importantly:

- pure code must not pay async scheduler overhead
- effectful code may suspend implicitly
- eager effect start is the default
- laziness is explicit through `defer`

## 3. Runtime Scope

The runtime is responsible for:

- program startup
- module initialization
- execution of effectful tasks
- implicit suspension and resumption
- generator state management
- native implementations of `std:` modules
- error propagation across runtime boundaries
- value representation and memory management strategy

The runtime is not responsible for:

- type inference
- parsing
- semantic analysis
- code formatting

## 4. Execution Model

FScript programs are expression-oriented and async-by-semantics.

Execution model:

- pure expressions evaluate immediately
- effectful calls start eagerly when reached
- values from effectful calls suspend implicitly when consumed
- effects preserve observable source ordering unless proven independent
- `defer expr` delays effect start intentionally

Example source:

```fs
something = (): String => {
  filepath = '/tmp/test.txt'
  content = getContent()
  FileSystem.writeFile(filepath, content)
  content
}
```

Runtime interpretation:

1. bind `filepath`
2. start `getContent()` immediately
3. suspend when `content` is needed but not ready
4. start `FileSystem.writeFile(filepath, content)` when dependencies are ready
5. resolve block result as `content`

## 5. Single-Threaded Scheduler

Draft 0.1 uses a single-threaded scheduler.

Reasons:

- simpler semantics
- easier determinism
- lower implementation complexity
- closer alignment with the language's effect-ordering rules

Scheduler responsibilities:

- manage ready tasks
- manage suspended tasks
- resume tasks when dependencies resolve
- preserve ordering for observable effects
- support explicit deferred tasks

The scheduler should not invent concurrency where the dependency/effect rules do not allow it.

## 6. Task Model

The runtime should represent effectful work explicitly.

Minimum task states:

- created
- ready
- running
- waiting
- completed
- failed
- canceled (reserved for future use)

Two core task categories:

- eager task: starts when execution reaches it
- deferred task: created by `defer` and starts only when forced or invoked

Recommended runtime rule:

- pure functions do not become tasks
- only effectful operations become scheduler-managed work

## 7. `defer`

`defer` is a runtime-visible construct.

Example:

```fs
a = defer getA()
b = defer getB()
```

Runtime requirements:

- `defer expr` captures the expression and its environment safely
- creating a deferred value does not start the effect
- forcing or invoking the deferred value starts the effect exactly once unless future semantics say otherwise
- repeated force should either memoize or be explicitly defined; Draft 0.1 should prefer memoized single-start semantics

Recommended behavior:

- `defer` returns a deferred task-like runtime value
- forcing it yields the same eventual result on repeated use

## 8. Generators

Generators are runtime-managed lazy sequences.

Runtime responsibilities:

- store generator state
- resume execution from the last `yield`
- produce next yielded value or completion
- preserve captured environments

Draft 0.1 rules:

- generators are for pure lazy iteration
- generators do not perform async streaming
- async streaming should be a separate future runtime abstraction

Recommended runtime representation:

- compiled/interpreted generator frame
- instruction pointer / state index
- captured locals
- completion flag

## 9. Values

The runtime needs a concrete value model.

Minimum runtime value categories:

- number
- string
- boolean
- null
- undefined
- record
- array
- function/closure
- generator
- deferred task
- effect task handle
- tagged union record values

High-level requirements:

- records are immutable
- arrays are immutable
- closures capture immutable environments
- runtime values should support structural equality where required by the language

Implementation guidance:

- start with a simple tagged value representation
- optimize only after semantic correctness is established
- keep pure-path representations friendly to AOT compilation later

## 10. Structural Equality

The runtime must support structural equality for plain data.

This includes:

- primitives by value
- arrays by element-wise comparison
- records by field-wise comparison
- tagged unions by structural comparison

Rules:

- functions are not comparable
- generators are not comparable
- streams are not comparable

Performance note:

- structural equality is semantically required
- implementation should avoid unnecessary deep comparisons when cheap short-circuits are available

## 11. Immutable Data Operations

Because mutation is disallowed, updates must create new values.

Runtime responsibilities include support for efficient forms of:

- record creation
- record merge/spread
- array creation
- array append/update producing new arrays

Draft 0.1 implementation guidance:

- begin with simple immutable copying semantics where needed
- optimize later with persistent or copy-on-write strategies if profiling justifies it

## 12. Standard Library Runtime Boundary

`std:` modules are provided by the runtime and standard library implementation, not by JavaScript globals or Node.js APIs.

Examples:

- `std:filesystem`
- `std:json`
- `std:logger`
- `std:array`
- `std:object`
- `std:result`
- `std:task`

Runtime responsibilities:

- register built-in modules
- expose native host implementations safely
- keep host boundaries narrow and typed where possible

Additional host-boundary requirements for Draft 0.1:

- `std:json` parsing must support relaxed comment-tolerant input for configuration-style files
- `std:logger` must write to the process terminal without depending on JavaScript console APIs
- pretty-printed JSON output should be stable so logs and snapshots remain predictable

## 13. Host Boundaries

Although FScript does not depend on a JavaScript runtime, it still interacts with the outside world through runtime capabilities.

Examples:

- file IO
- process IO
- clock/time
- randomness
- network IO
- terminal logging

Rules:

- host boundaries are where runtime checks and validation may happen
- internal well-typed FScript execution should not repeatedly re-check already-proven types
- boundary failures should become `Result` values or exceptional runtime failures according to the standard-library contract

## 14. Error Propagation

The runtime must support both:

- expected failure values such as `Result<T, E>`
- exceptional failures via `throw`

Runtime behavior:

- `Result` is just ordinary data from the runtime's perspective
- thrown values unwind to the nearest `catch` expression
- uncaught thrown values terminate execution with a clear runtime error report
- thrown values are plain data, not class instances

## 15. Module Initialization

Each source file is a module.

Runtime/module-loader responsibilities:

- load each module once
- initialize imports before dependent execution
- execute top-level module code once
- reject circular imports before runtime execution in Draft 0.1

For compiled binaries, this can be resolved mostly at compile/link time.

For interpreter mode, the runtime/driver should still preserve the same semantics.

## 16. Startup Model

`fscript run file.fs`

Recommended flow:

1. parse and typecheck the entry module graph
2. lower to IR
3. initialize runtime
4. load std modules
5. initialize program modules
6. execute the entrypoint module/block
7. drain the scheduler until completion
8. report success or failure

`fscript compile input.fs output`

Recommended flow:

1. parse and typecheck the entry module graph
2. lower to IR
3. lower to native code
4. link with runtime support
5. emit executable

## 17. Interpreter and Native Runtime Relationship

The interpreter and compiled runtime must share one semantic contract.

Recommended rule:

- IR is the semantic boundary
- interpreter executes IR directly
- native codegen lowers the same IR into machine code plus runtime calls

This reduces divergence risk.

## 18. Memory Management Strategy

Draft 0.1 does not need to lock in a final GC strategy immediately, but it must define constraints.

Requirements:

- values live long enough for closures, generators, and suspended tasks
- immutable records and arrays are safe to share
- memory management must be compatible with native compilation

Recommended path:

- begin with a straightforward owned/shared runtime representation in Rust
- avoid premature GC design complexity
- introduce a dedicated runtime allocator or tracing strategy later only if profiling demands it

## 19. Performance Goals

The runtime should enable better predictability than JavaScript engines by taking advantage of:

- fixed semantics
- immutability
- no prototype chain
- no hidden-class churn
- no implicit `any`
- compile-time typing

Runtime-specific performance goals:

- pure code should execute without task allocation overhead
- effectful code should incur scheduler overhead only where necessary
- generator stepping should be cheap and explicit
- host-boundary calls should be measurable and narrow
- startup should be fast in both interpreter and compiled modes

## 20. Observability

The runtime should support debugging and profiling from early on.

Recommended features:

- stable stack traces in interpreter mode
- source spans attached to runtime errors where possible
- tracing hooks for scheduler activity
- optional debug logging for task state transitions
- benchmark-friendly instrumentation points

## 21. Testing Requirements

The runtime must be tested directly and through end-to-end programs.

Required runtime test categories:

- scheduler ordering tests
- eager effect start tests
- `defer` tests
- generator resume/completion tests
- structural equality tests
- immutable record/array behavior tests
- std module host-boundary tests
- uncaught throw behavior tests
- interpreter vs compiled parity tests

The runtime implementation must participate in the project-wide 100% coverage gate.

## 22. Implementation Guidance for Rust

Recommended crate boundaries:

- `fscript-runtime` for core runtime values, scheduler, and intrinsics
- `fscript-std` for std-module implementations against the runtime interface
- `fscript-interpreter` for IR execution
- `fscript-codegen-cranelift` for native lowering that reuses runtime contracts

Recommended Rust design principles:

- keep runtime data structures explicit
- keep unsafe code out unless absolutely necessary and justified by profiling
- keep runtime/public APIs small and testable
- favor enums and small focused structs over deep abstraction layers early on

## 23. Summary

The FScript runtime is a native Rust runtime with:

- single-threaded scheduling
- eager async semantics for effects
- native `defer`
- pure lazy generators
- immutable data
- runtime-backed `std:` modules
- no JavaScript engine dependency
