# FScript Implementation Plan

Status: Draft 0.1

## 1. Goal

Build `fscript` in Rust as a native compiler toolchain for the FScript language described in `specs/LANGUAGE.md`.

The initial product must support:

- `fscript run somefile.fs`
- `fscript compile somefile.fs somefile`
- a native Rust runtime
- no JavaScript runtime dependency
- no JavaScript compilation target requirement
- strict static typing
- immutable bindings and immutable data semantics
- async-by-semantics with eager effect start and native `defer`
- generators, `yield`, `match`, destructuring, and data-last `std:` modules

This plan should be read alongside:

- [LANGUAGE.md](/Users/markwylde/Documents/Projects/fscript/specs/LANGUAGE.md)
- [GRAMMAR.md](/Users/markwylde/Documents/Projects/fscript/specs/GRAMMAR.md)
- [TYPESYSTEM.md](/Users/markwylde/Documents/Projects/fscript/specs/TYPESYSTEM.md)
- [RUNTIME.md](/Users/markwylde/Documents/Projects/fscript/specs/RUNTIME.md)
- [STDLIB.md](/Users/markwylde/Documents/Projects/fscript/specs/STDLIB.md)
- [NATIVE_ABI.md](/Users/markwylde/Documents/Projects/fscript/specs/NATIVE_ABI.md)
- [CODE_STYLE.md](/Users/markwylde/Documents/Projects/fscript/specs/CODE_STYLE.md)

The project should be built in layers so we get a working language quickly, then harden it without architectural rewrites.

## 1.1 Current Repository Status

The repository has passed the scaffolding stage and now has a broad parseable frontend plus a useful bootstrap execution bridge.

Implemented today:

- Cargo workspace and crate layout
- `fscript-cli` command wiring for `run`, `compile`, and `check`
- shared source loading, spans, and line/column mapping
- a working handwritten lexer with diagnostics
- a broad handwritten parser for imports, exports, types, patterns, functions, blocks, records, arrays, calls, pipes, control flow, and generator syntax
- a first real semantic frontend slice: resolved HIR, AST-to-HIR lowering, pipe lowering, name resolution, and strict typechecking for the currently supported executable subset
- a first real effect-analysis slice that classifies current callables as `Pure`, `Effectful`, or `Deferred` and rejects effectful generator work
- `fscript check` validating lexing, parsing, name resolution, typechecking, effect analysis, and user-module import graphs for the currently supported frontend
- a first real shared execution layer: executable IR, HIR-to-IR lowering, a shared runtime value model, runtime-backed `std:` modules, and an IR interpreter for the current executable subset
- shared-interpreter support for user-defined functions, currying, records, arrays, `if`, `match`, destructuring, generators, `try/catch`, `throw`, and lazy memoized `defer`
- runtime support for user-module imports with canonical path resolution, cycle rejection, and once-per-module initialization
- runtime-backed `std:json`, `std:filesystem`, a minimal `std:http`, and an expanded shared-runtime `std:task` surface (`Task.all`, `Task.race`, `Task.spawn`, `Task.defer`, `Task.force`)
- explicit deferred-task state tracking in the shared runtime (`created`, `ready`, `running`, `waiting`, `completed`, `failed`) with memoized outcomes shared across the interpreter and stdlib task helpers
- a first shared single-threaded scheduler abstraction in `fscript-runtime` that now owns deferred/task state transitions and is used by the interpreter for deferred execution instead of interpreter-local task orchestration
- scheduler-backed execution for ordinary effectful native stdlib calls now flows through deferred runtime handles as well, and those calls now start eagerly when reached while still memoizing results for later consumption
- `fscript run` executing the shipped example set through the semantic frontend plus shared IR/interpreter path
- a bootstrap native compiler backend that now emits standalone executables from the shared IR program graph, not just a single lowered module
- a first bounded real Cranelift backend slice for the numeric single-module subset that lowers shared IR to Cranelift IR, emits native object files, and links executables through the system toolchain plus a tiny runtime-print shim
- that bounded Cranelift slice now crosses the native boundary through runtime-owned value handles for its final numeric results instead of an ad hoc raw-`double` print helper, aligning the shipped subset with the documented Draft 0.1 ABI direction
- a fixed interpreter-backed embedded-runner compile bridge that packages the shared IR graph into emitted executables so compiled programs can inherit imports, functions, generators, `try/catch`, `throw`, `defer`, user modules, and the current shared-runtime stdlib surface from the active `run` path without generating ad hoc Rust source per program
- the mixed compile pipeline still includes the earlier plain-data/control-flow slice for literals, blocks, records, arrays, non-`defer` unary operators, `if`, structural equality, member access, and index access, and now has broader parity tests against `run`

Still missing or only placeholder:

- the shared runtime now covers deferred/task execution plus eager-start ordinary host IO handles, but it still does not provide a long-lived dependency-driven scheduler that can keep background work draining across a whole evaluation
- the real native codegen path currently covers only the first bounded numeric single-module subset; the broader compile path still falls back to the embedded-runner bridge
- compile parity is now much closer to `run`, but the project still needs the final shared native runtime/codegen contract across the broader interpreter-backed compile subset instead of a mixed native-plus-embedded-runner bridge

Bridge-runtime status:

- the parser is still ahead of the full runtime surface, but `run` now flows through shared IR plus the interpreter crate instead of the old bridge
- the old driver bridge remains historical/bootstrap infrastructure, not the active `run` path
- the shared runtime currently covers the pure/example-backed subset, scheduler-backed deferred/task evaluation, explicit task helpers, and eager ordinary effect start for the currently implemented host operations
- the remaining runtime gap is long-lived/background dependency draining across an entire evaluation, not the basic eager-start task/effect model

Implementation note for the next slice:

- keep the shared IR/interpreter path as the source of truth for `run`
- avoid growing the legacy bridge unless it is required temporarily for diagnostics or bootstrap compile coverage
- keep `fscript compile` explicitly documented as a mixed native pipeline for now: a narrow real Cranelift path exists, but the embedded-runner bridge still owns the broader subset until parity expands

The next correct implementation step is therefore to keep building on the shared execution slice in two tracks:

- extend the scheduler from eager-start handles into a long-lived dependency-driven runtime that can keep background work draining across a whole evaluation
- expand the new bounded Cranelift/object/link path from the numeric single-module slice toward the full shared compile subset, then retire the embedded-runner bridge

Near-term compile work should stay focused on the shared program graph:

- keep `compile` consuming the same fully loaded IR module graph as `run`
- keep broadening parity through the shared interpreter/runtime bridge only when it materially improves user-facing behavior
- avoid reintroducing a second ad hoc execution semantics just for the compile bridge

## 2. Implementation Principles

We should optimize for:

- correctness first
- simple architecture first
- native performance second
- feature growth without rewrites
- excellent diagnostics from day one
- aggressive automated testing from day one

Important constraints:

- reject ill-typed programs before execution
- avoid runtime type checks inside well-typed FScript code
- keep pure execution paths free of async scheduler overhead
- lower only effectful code into suspendable runtime machinery
- preserve a clean boundary between pure semantics and host/runtime capabilities

## 3. Non-Negotiable Build Rules

These rules are intended to keep the implementation incremental and avoid speculative architecture.

- Every milestone must produce a runnable, testable slice, not just library scaffolding.
- Crate APIs should be introduced only when a concrete consumer exists.
- We should implement one semantic boundary at a time: syntax before types, types before effects, effects before runtime scheduling, interpreter before native codegen.
- Unsupported syntax must fail clearly and early.
- Runtime capabilities must remain behind explicit interfaces; parser and typechecker crates must never know about host IO details.
- Parser, typechecker, interpreter, and codegen must share source spans and diagnostic conventions.

## 4. Recommended Rust Stack

Use modern stable Rust with the Rust 2024 edition.

Recommended core tooling:

- language: Rust 2024
- workspace management: Cargo workspace
- CLI: `clap`
- lexer: `logos` or a small handwritten lexer if it remains simpler and fully tested
- parser: handwritten recursive descent parser with Pratt parsing for expressions
- diagnostics: `miette`
- snapshot tests: `insta`
- property tests: `proptest`
- coverage gate: `cargo-llvm-cov`
- code generation backend: `cranelift`
- object emission for binaries: `cranelift-module` plus `cranelift-object`

Recommended support crates:

- `thiserror` for internal Rust error types
- `camino` for UTF-8 path handling
- `serde` only where it materially helps internal tooling or test fixtures
- `smallvec` where profiling shows it helps AST or IR allocations
- `tracing` for internal compiler and runtime instrumentation

### Why these choices

`clap`
: Mature and standard for Rust CLIs.

`logos`
: Very fast lexer generation, a good fit for a compiler front-end.

Handwritten lexer
: Also acceptable if it stays small, explicit, and well-tested. The current repository already has a handwritten lexer, so lexer replacement is not a prerequisite for frontend progress.

Handwritten parser
: Better control over custom grammar, precedence, recovery, and language evolution than a parser generator for a language like FScript.

`miette`
: Good compiler-style diagnostics with source spans and helpful rendering.

`insta`
: Excellent for snapshotting diagnostics, formatted ASTs, IR, and CLI output.

`proptest`
: Ideal for parser, typechecker, and optimizer invariants.

`cargo-llvm-cov`
: Best current fit for enforcing strict Rust coverage gates in CI.

`cranelift`
: Pure-Rust native code generation with a strong fit for a Rust-native toolchain.

## 5. High-Level Architecture

The workspace and crate boundaries should reflect the spec split:

- language surface and semantics from [LANGUAGE.md](/Users/markwylde/Documents/Projects/fscript/specs/LANGUAGE.md)
- concrete syntax from [GRAMMAR.md](/Users/markwylde/Documents/Projects/fscript/specs/GRAMMAR.md)
- typing rules from [TYPESYSTEM.md](/Users/markwylde/Documents/Projects/fscript/specs/TYPESYSTEM.md)
- runtime behavior from [RUNTIME.md](/Users/markwylde/Documents/Projects/fscript/specs/RUNTIME.md)
- standard-library APIs from [STDLIB.md](/Users/markwylde/Documents/Projects/fscript/specs/STDLIB.md)
- Rust implementation conventions from [CODE_STYLE.md](/Users/markwylde/Documents/Projects/fscript/specs/CODE_STYLE.md)

Build the compiler and runtime as a Cargo workspace.

Suggested workspace layout:

```text
fscript/
  crates/
    fscript-cli/
    fscript-source/
    fscript-lexer/
    fscript-parser/
    fscript-ast/
    fscript-hir/
    fscript-types/
    fscript-effects/
    fscript-lower/
    fscript-ir/
    fscript-runtime/
    fscript-interpreter/
    fscript-codegen-cranelift/
    fscript-driver/
    fscript-std/
    fscript-test-support/
  examples/
  specs/
```

Responsibilities:

`fscript-cli`
: command-line UX and subcommands.

`fscript-source`
: source files, spans, file IDs, path resolution, source maps, and shared diagnostic file content.

`fscript-lexer`
: tokenization and trivia handling.

`fscript-parser`
: CST/AST parsing, error recovery, syntax diagnostics.

`fscript-ast`
: parsed syntax tree structures.

`fscript-hir`
: lowered semantic tree after name resolution and desugaring.

`fscript-types`
: type inference and typechecking.

`fscript-effects`
: purity/effect analysis and eager-vs-deferred effect lowering metadata.

`fscript-lower`
: AST to HIR, HIR to IR, pipe lowering, currying lowering, match lowering.

`fscript-ir`
: compiler IR suitable for interpretation and native codegen.

`fscript-runtime`
: native runtime, scheduler, effect execution, std module host functions.

`fscript-interpreter`
: execute IR directly for `fscript run` during early milestones.

`fscript-codegen-cranelift`
: emit native code and object files.

`fscript-driver`
: orchestration layer used by CLI and tests.

`fscript-std`
: definition and host implementation of `std:` modules.

`fscript-test-support`
: test fixtures, golden helpers, example runners, coverage helpers.

### 5.1 Layering contract

This dependency direction is mandatory:

- `fscript-source` is a foundational crate and must not depend on compiler frontend or runtime crates.
- `fscript-ast` may depend on `fscript-source`.
- `fscript-lexer` may depend on `fscript-source`.
- `fscript-parser` may depend on `fscript-source`, `fscript-lexer`, and `fscript-ast`.
- `fscript-hir` may depend on `fscript-source`.
- `fscript-types` may depend on `fscript-source` and `fscript-hir`.
- `fscript-effects` may depend on `fscript-source` and `fscript-hir`.
- `fscript-lower` may depend on `fscript-ast`, `fscript-hir`, `fscript-types`, `fscript-effects`, and `fscript-ir`.
- `fscript-runtime` must not depend on parser or AST crates.
- `fscript-std` may depend on `fscript-runtime`.
- `fscript-interpreter` may depend on `fscript-ir`, `fscript-runtime`, and `fscript-std`.
- `fscript-codegen-cranelift` may depend on `fscript-ir` and `fscript-runtime`.
- `fscript-driver` may depend on all user-facing compiler crates and is the main orchestration seam.
- `fscript-cli` should remain thin and depend mainly on `fscript-driver`.

Forbidden patterns:

- runtime depending on parser, AST, or CLI crates
- typechecker depending on runtime
- source/span infrastructure depending on diagnostics renderers or CLI concerns
- direct codegen shortcuts that bypass shared IR semantics

## 6. Execution Strategy

We should not begin with native codegen.

Instead, build in this order:

1. workspace bootstrap and shared source/span infrastructure
2. lexer
3. parser
4. AST
5. name resolution
6. typechecking
7. effect analysis
8. lowering to IR
9. interpreter for `fscript run`
10. native code generation for `fscript compile`

This gets the language working earlier and reduces the risk of debugging frontend and backend failures at the same time.

### 6.1 Bounded phase gates

The implementation should be divided into explicit gates.

Phase 0: workspace bootstrap

- Cargo workspace exists
- `fscript` CLI parses commands
- shared source/span types exist
- test infrastructure and formatting are wired

Phase 1: frontend-only vertical slice

- `fscript check` loads `.fs` files
- lexer produces tokens, trivia, and lexical diagnostics
- parser produces AST with spans
- parser recovery yields multiple diagnostics in one pass
- no runtime, effect system, or codegen yet

Phase 2: full parseable frontend

- imports and exports
- type declarations and type syntax
- functions and generator arrows
- records, arrays, calls, member access, indexing, unary/binary operators, and pipes
- blocks and control-flow expressions
- destructuring and match patterns
- `fscript check` validates lexing plus parsing for the supported module graph

Phase 3: semantic frontend

- module resolution
- name resolution
- typechecking
- effect inference metadata
- HIR lowering

Phase 4: executable interpreter slice

- IR exists
- runtime exists
- interpreter executes the supported subset
- `std:` module contract is live
- `fscript run` works for the supported subset

Phase 5: native compilation

- codegen lowers shared IR
- object emission and linking work
- `fscript compile` emits executables

Phase 6: hardening

- diagnostics refinement
- parity testing
- coverage enforcement
- benchmarks and performance tuning

### 6.2 First implementation slice

The first slice we should implement now is intentionally narrow:

- workspace bootstrap
- shared source/span infrastructure
- thin CLI wiring
- initial driver
- lexer
- the minimum parser skeleton needed to support `fscript check`

This slice is the best tradeoff between immediate value and low rewrite risk because it locks in spans, diagnostics, tokens, AST shape, and crate boundaries before the type and runtime work begins.

### 6.3 Current implementation slice

The parser-oriented slice, the semantic-plus-effect slice, and the first shared execution slice are now effectively done.

`fscript run` already flows through the shared frontend, shared IR, shared runtime, and IR interpreter for the current supported language subset. The repository also has:

1. user-module loading with canonical path resolution and cycle rejection
2. runtime-backed execution for `try/catch`, `throw`, `defer`, generators, and the current `std:` surface
3. a shared single-threaded scheduler that owns deferred/task state transitions and eager-start ordinary host calls
4. a mixed `compile` path with a bounded real Cranelift backend plus the broader embedded-runner bridge

The next slice is therefore hardening, not another architectural reset:

1. keep the current HIR, typechecking, effect-analysis, IR, runtime, and interpreter path as the single source of truth for `run`
2. expand scheduler coverage from eager-start task handles toward a longer-lived dependency-driven runtime that can keep background work draining across a whole evaluation
3. keep broadening the real Cranelift-owned subset until it replaces the embedded-runner fallback for the broader compile surface
4. drive the remaining work through coverage, parity tests, and diagnostics hardening instead of adding a second execution path

This keeps the repository in an honest state: the language already parses, typechecks, effect-checks, and executes a broad meaningful subset, so the remaining work is to close hardening gaps and retire fallback paths rather than to prove the execution model from scratch.

### 6.4 Bootstrap executable bridge

The original bootstrap executable bridge is now historical context rather than the active plan of record.

What remains in practice is:

- the shared interpreter/runtime path is the source of truth for `run`
- the embedded-runner bridge still exists only inside the mixed `compile` pipeline so compiled executables can preserve broader parity while the real Cranelift subset expands
- backend work should keep shrinking that compile-time fallback rather than extending any interpreter-less bootstrap executor

Acceptance criteria for the remaining bridge work:

- every shipped example in `examples/` continues to run successfully through `fscript run`
- the mixed `compile` path stays explicit about which programs are owned by the real Cranelift backend versus the embedded-runner fallback
- fallback behavior does not introduce a second user-visible execution semantics

## 7. Frontend Design

### 7.1 Lexer

The current repository already has a handwritten lexer, and that is acceptable for now.

The important requirement is that the lexer satisfies the Draft 0.1 lexical contract, preserves spans, and stays comprehensively tested. Replacing it with `logos` is an optimization or maintenance decision, not a prerequisite for the rest of the frontend.

Decision for Draft 0.1: keep the handwritten lexer. Revisit replacement only if later maintenance or measured performance data justifies it after the frontend freezes.

The lexer must tokenize:

- identifiers
- keywords
- punctuation
- operators
- numbers
- strings
- comments
- trivia

Requirements:

- preserve spans for all tokens
- preserve comments and trivia well enough for diagnostics and future formatting tools
- emit invalid-token diagnostics without panicking

The lexer must cover these Draft 0.1 lexical categories:

- keywords: `import`, `from`, `export`, `type`, `if`, `else`, `match`, `try`, `catch`, `throw`, `defer`, `yield`, `true`, `false`
- reserved built-in type names: `Number`, `String`, `Boolean`, `Null`, `Undefined`, `Never`, `Unknown`
- operators: `=`, `=>`, `|>`, `||`, `&&`, `??`, `===`, `!==`, `<`, `<=`, `>`, `>=`, `+`, `-`, `*`, `/`, `%`, `!`, `|`, `&`, `.`, `:`, `,`
- punctuation: `(`, `)`, `{`, `}`, `[`, `]`
- literals: numbers, strings, booleans, `Null`, `Undefined`
- trivia: whitespace, line comments, block comments

String rules for the first implementation slice:

- support single-quoted and double-quoted strings
- support common escapes needed by examples and tests
- report unterminated strings and invalid escapes as lexical diagnostics
- preserve the original lexeme span for downstream reporting

### 7.2 Parser

Use a handwritten parser with:

- recursive descent for declarations and block items
- Pratt parsing for expressions and operator precedence
- explicit parse functions for arrow functions, generator arrows, `match`, `if`, `try/catch`, destructuring, and pipes

The parser should support error recovery well enough to continue after local syntax errors and produce multiple diagnostics in one pass.

The parser should reject unsupported legacy-style forms explicitly:

- `function`
- `class`
- `interface`
- `enum`
- `let`
- `const`
- `var`
- `return`
- `new`
- `this`

### 7.3 AST and HIR split

Do not typecheck directly against raw AST.

Recommended flow:

- AST for syntax shape
- HIR for resolved semantics

HIR should desugar:

- data-last pipes
- implicit currying metadata
- block final-expression returns
- generator arrows
- `match`
- destructuring
- export/import normalization

### 7.4 Module resolution

Draft 0.1 module resolution rules should be fixed before semantic work begins.

- Entry points must be `.fs` files.
- Relative imports resolve from the importing file's directory.
- User modules must include the `.fs` extension explicitly in source.
- `std:` module paths are reserved and are resolved by the driver/runtime, not the filesystem.
- The driver should canonicalize filesystem paths before inserting modules into the graph.
- A module graph node should be loaded and parsed once per canonical path.
- Circular imports are a compile error detected before runtime execution begins.

## 8. Type System Plan

Implement the typechecker before any optimization work.

This section should follow [TYPESYSTEM.md](/Users/markwylde/Documents/Projects/fscript/specs/TYPESYSTEM.md).

Required type features for v0.1:

- primitives
- arrays
- records
- function types
- tagged unions
- generics
- structural equality checks for type compatibility
- `Never`
- `Unknown`
- `Result<T, E>` as a standard-library type, not a language primitive

Typechecker rules:

- reject ill-typed programs before run or compile
- no implicit `any`
- no runtime recovery from internal type errors
- full typechecking for module boundaries
- effectful functions participate in typechecking and effect analysis together

We should treat exported functions as API surfaces and ensure diagnostics clearly show inferred types and inferred effects.

## 9. Effect System and Async Semantics Plan

This section should follow [LANGUAGE.md](/Users/markwylde/Documents/Projects/fscript/specs/LANGUAGE.md) and [RUNTIME.md](/Users/markwylde/Documents/Projects/fscript/specs/RUNTIME.md).

The effect analyzer must classify code into:

- pure
- eager effectful
- deferred effectful

Draft 0.1 should use a simple effect lattice:

- `Pure`
- `Effectful`
- `Deferred`

Language rules to implement:

- pure expressions execute immediately
- effectful calls start eagerly when reached
- values from effectful calls suspend implicitly when consumed
- `defer expr` delays effect start until force or invocation
- pure code must not be lowered into scheduler tasks

Implementation approach:

- annotate HIR and IR nodes with purity/effect metadata
- create explicit IR nodes for eager call, deferred call, suspend point, and force
- preserve source order for observable effects
- allow overlap only when dependency analysis proves it is safe
- record effect metadata on exported functions for diagnostics and future tooling

The runtime scheduler should be single-threaded in v0.1.

## 10. Intermediate Representation Plan

Use a custom IR instead of lowering directly from HIR into Cranelift.

IR goals:

- compact and serializable enough for debugging tools
- explicit control flow
- explicit effect boundaries
- explicit block results
- explicit generator state transitions
- explicit pattern-match branching
- explicit closure environments
- explicit immutable record and array operations

Minimum IR features:

- constants
- local bindings
- function definitions
- function application
- closure capture
- block expression
- branch
- match dispatch
- call to runtime intrinsic
- eager effect call
- deferred effect thunk
- yield/generator step
- record construction
- array construction
- structural equality op

### 10.1 Currying and pipe lowering

Before implementing the lowerer we should adopt one canonical representation:

- source calls keep their original source arity in AST
- HIR normalizes function definitions with arity metadata
- partial application becomes an explicit HIR or IR node rather than an implicit nested closure rewrite everywhere
- pipes lower into ordinary call nodes with the piped value inserted as the final argument

This keeps diagnostics readable and makes both interpreter and codegen simpler to optimize later.

## 11. Runtime Plan

This section should follow [RUNTIME.md](/Users/markwylde/Documents/Projects/fscript/specs/RUNTIME.md).

`fscript-runtime` should be a small, native runtime in Rust.

Responsibilities:

- single-threaded scheduler
- async task representation
- deferred task representation
- generator runtime state
- immutable record and array value model
- runtime intrinsics for `std:` modules
- entrypoint loading for `run` and compiled binaries

Key design goals:

- pure calls stay as direct native calls whenever possible
- effectful calls go through a minimal scheduler API
- runtime allocations are visible and measurable
- host boundaries are narrow and explicit

Native-runtime work required before the embedded runner can be retired:

- define a stable runtime ABI between Cranelift-generated code and `fscript-runtime` for values, closures, generators, deferred handles, task handles, throws, and module records
- split the current runtime contract into two tiers:
  - a low-level native ABI callable from generated code
  - a higher-level Rust convenience API used by the interpreter and stdlib tests
- decide and document the native ownership model for values that cross generated-code boundaries so closures, generators, deferred tasks, arrays, and records can outlive individual stack frames safely
- make module initialization runtime-visible in the native path so compiled executables preserve the same once-per-module semantics, dependency ordering, and cycle rejection as `run`
- extend the scheduler so native-generated effectful code can create, wait on, resume, and force runtime work without going back through the interpreter
- add runtime conformance tests that execute the same scenarios through interpreter entrypoints and native ABI entrypoints

Current Draft 0.1 ABI choice:

- stable runtime boundary: opaque runtime-owned handles for every value category
- optimized internal lowering may still use unboxed primitives inside generated code for hot pure paths
- current shared metadata for that contract lives in `fscript-runtime`, and the narrative contract is documented in [NATIVE_ABI.md](/Users/markwylde/Documents/Projects/fscript/specs/NATIVE_ABI.md)

## 12. Standard Library Plan

This section should follow [STDLIB.md](/Users/markwylde/Documents/Projects/fscript/specs/STDLIB.md).

Implement `std:` modules in Rust as runtime-backed capabilities.

Required early modules:

- `std:array`
- `std:object`
- `std:string`
- `std:number`
- `std:result`
- `std:json`
- `std:logger`
- `std:filesystem`
- `std:task`

Guidelines:

- std APIs must be data-last where appropriate
- std functions should be curried consistently with language rules
- std modules should be testable independently of the full compiler
- std modules must obey FScript immutability semantics

### 12.1 Standard library contract

Before implementing host functions we should define:

- each `std:` module's exported symbol table
- whether a function is pure, effectful, or deferred-capable
- whether failures surface as `Result` values or exceptional runtime failures
- the runtime registration interface used by interpreter and codegen
- conformance tests that assert stable signatures and semantics

Native-compile follow-up required for stdlib parity:

- classify every exported stdlib function as one of:
  - direct pure intrinsic lowered inline or via a tiny runtime helper
  - runtime call with no suspension
  - scheduler-aware effectful runtime call
- implement the pure/native-first stdlib surface in the Cranelift path before growing fallback-dependent examples:
  - `std:array`: `length`, `append`, `concat`, `at`, `slice`, then `map`/`filter`/`reduce`/`flatMap`
  - `std:object`: `spread`, `keys`, `values`, `entries`, `has`, `get`, `set`
  - `std:string`: `length`, `uppercase`, `lowercase`, `trim`, `split`, `join`, `startsWith`, `endsWith`, `contains`, `isDigits`
  - `std:number`: `parse`, `toString`, `floor`, `ceil`, `round`, `min`, `max`, `clamp`
  - `std:result`: constructors and structural helpers, then higher-order helpers like `map`, `mapError`, `andThen`
- implement the host-boundary stdlib surface through explicit runtime calls with native tests for:
  - `std:json`
  - `std:logger`
  - `std:filesystem`
  - `std:task`
- add a backend-owned parity matrix for every stdlib export so each function is marked as:
  - interpreter-only
  - native via runtime call
  - native lowered/inlined

Current backend parity table:

| Module | Export | Current compile owner |
| --- | --- | --- |
| `std:array` | `map`, `filter`, `length` | embedded runner |
| `std:object` | `spread` | embedded runner |
| `std:string` | `trim`, `uppercase`, `lowercase`, `isDigits` | embedded runner |
| `std:number` | `parse` | embedded runner |
| `std:result` | `ok`, `error`, `isOk`, `isError`, `withDefault` | embedded runner |
| `std:json` | `jsonToObject`, `jsonToString` | embedded runner |
| `std:json` | `jsonToPrettyString` | native via runtime call |
| `std:logger` | `create`, `log`, `debug`, `info`, `warn`, `error`, `prettyJson` | embedded runner |
| `std:filesystem` | `readDir` | embedded runner |
| `std:filesystem` | `readFile`, `writeFile`, `exists`, `deleteFile` | native via runtime call |
| `std:task` | `all`, `race`, `spawn`, `defer`, `force` | embedded runner |
| `std:http` | `serve` | embedded runner |

## 13. Interpreter Plan

Build `fscript run` on top of the IR interpreter first.

Why:

- shortest path to executing real programs
- best debugging surface while semantics are still moving
- simpler validation for typechecking, effects, generators, and `match`

Interpreter responsibilities:

- evaluate IR
- call runtime intrinsics
- manage effect suspension
- run generators
- support source-mapped stack traces and diagnostics

The interpreter should remain even after native compilation exists, because it will continue to power tests, debugging, and fast semantic iteration.

The repository now has that first interpreter slice. The remaining bridge code in `fscript-driver` should be treated as disposable bootstrap infrastructure and should not be extended ahead of the shared IR/runtime/interpreter path.

## 14. Native Compilation Plan

Build `fscript compile` after the interpreter is stable.

Bridge strategy before Milestone 8:

- while the repository still lacks a real Cranelift-backed native pipeline, `fscript compile` may use a clearly documented bootstrap backend
- the bootstrap backend should accept only the currently supported execution subset
- it should reuse the same frontend validation as `run` and fail early with clear diagnostics when a source file uses unsupported constructs
- it should still emit a native standalone executable so example programs like `hello_world.fs` can participate in end-to-end compile coverage now
- once IR and runtime are in place, replace this backend with the planned Cranelift pipeline rather than extending the bridge indefinitely

Current repository note:

- `crates/fscript-codegen-cranelift` now uses a mixed backend: a bounded real Cranelift path for the numeric single-module subset plus a fixed embedded-runner bridge for the broader interpreter-backed subset
- the older generated Rust-source bootstrap compiler has been removed
- the next honest native-compilation milestone is therefore not "finish codegen" in one jump, but to keep expanding the real Cranelift-owned subset while the embedded runner preserves broader parity
- those stages should be visible in the checklist so backend progress can be tracked without overstating parity
- the target end state for Draft 0.1 is that the embedded-runner path is removed from normal `fscript compile` output, not merely hidden behind a best-effort fallback

Recommended backend:

- lower FScript IR into Cranelift IR
- use `cranelift-object` to emit object files
- link objects into native executables through the system linker in v0.1

Compilation stages:

1. parse and typecheck
2. lower to HIR
3. lower to IR
4. run validation passes
5. lower to Cranelift IR
6. emit object file
7. link with `fscript-runtime`
8. produce executable

Native compilation goals:

- fast startup
- no JavaScript runtime dependency
- predictable execution
- simple debug info story first, better debug info later
- interpreter/native semantic parity through one shared IR/runtime contract
- no interpreter payload embedded in ordinary compiled executables

Required native backend expansion tracks:

1. Runtime ABI and value model
2. Pure IR lowering
3. Control-flow lowering
4. Function and closure lowering
5. Aggregate data lowering
6. Generator lowering
7. Deferred/task/effect lowering
8. User-module initialization and linking
9. Stdlib parity and host-boundary calls
10. Embedded-runner retirement

### 14.1 Native parity work breakdown

Track A: runtime ABI, calling convention, and ownership

- define the concrete ABI used by generated code for:
  - numbers, booleans, null, undefined
  - heap values such as strings, arrays, records, closures, generators, deferred handles, and task handles
  - function entrypoints, curried partial applications, and native stdlib call shims
  - thrown-value propagation and catch boundaries
- document whether the native path will use:
  - a boxed tagged `Value`
  - partially unboxed primitives plus boxed aggregates
  - a hybrid ABI with specialized signatures for hot pure paths
- add ABI-level tests that compile tiny generated stubs and assert round-tripping through runtime helpers

Track B: pure expression lowering

- expand the current numeric-only lowering to support:
  - strings
  - booleans
  - null and undefined
  - structural equality
  - records
  - arrays
  - member access
  - index access
  - `if`
  - `match`
  - destructuring
- keep unsupported forms failing in the frontend/codegen boundary with source-local diagnostics instead of silently falling back when the plan says a construct should now be native-owned

Track C: functions, currying, and closures

- lower user-defined function values into native callable objects
- lower partial application without routing through the interpreter
- preserve closure capture semantics and immutable environments
- add native parity tests for:
  - direct calls
  - partial application
  - higher-order stdlib calls
  - nested closures
  - cross-module function imports and exports

Track D: generators

- define a native generator frame layout that matches the runtime spec:
  - instruction pointer/state index
  - captured locals
  - yielded value slot
  - completion flag
- lower `yield` and resume points into explicit generator state machines
- ensure generators remain pure-lazy and reject effectful generator work in the same places as the interpreter
- add parity tests for creation, stepping, exhaustion, and captured-environment behavior

Track E: deferred work, tasks, and eager effects

- lower native `defer` into runtime-managed deferred handles instead of interpreter-only deferred bodies
- lower eager effect start into explicit scheduler/task creation at reach-time
- preserve the ordering rules from `LANGUAGE.md` and `RUNTIME.md` for:
  - eager effect start
  - dependency-driven suspension
  - forcing deferred work
  - `Task.all`
  - `Task.race`
  - `Task.spawn`
- add parity tests that compare observable ordering between `run` and native-compiled binaries

Track F: modules and linking

- lower the full loaded IR module graph into native-owned module initialization functions
- emit one native initialization unit per source module or an equivalent linked plan with deterministic init ordering
- preserve:
  - once-per-module top-level execution
  - import dependency ordering
  - user-module exports
  - std-module imports
  - circular import rejection before execution
- add compile-time fixtures with multiple user modules, re-exports, and mixed std/user imports

Track G: stdlib parity

- move pure stdlib helpers off the interpreter-backed path first
- then move scheduler-aware host functions onto explicit runtime call shims
- keep `Unknown`-producing and host-validation behavior concentrated at boundary APIs such as JSON and filesystem
- add one parity table in this plan mapping each stdlib export to its backend status until the table is empty

Track H: removing the embedded runner

- add an opt-in debug escape hatch if needed, but stop treating the embedded runner as the default success path for `compile`
- fail CI if a program expected to be native-owned still routes through the embedded-runner bridge
- delete the bridge only after:
  - native parity covers all supported examples
  - backend error diagnostics remain snapshot-stable
  - compiled executables no longer include the interpreter/program-image payload
### 14.2 Bounded backend replacement plan

Replace the bootstrap compiler in small, testable stages:

1. implement Cranelift lowering for the current pure parity subset of IR
2. emit native object files directly with `cranelift-module` and `cranelift-object`
3. link those objects into executables through the driver
4. expand the Cranelift-supported subset through pure aggregates, control flow, and structural equality
5. lower user functions, currying, and closures without interpreter participation
6. lower generators, deferred values, eager effects, and scheduler-aware stdlib calls
7. lower the full module graph and stdlib imports to the native runtime contract
8. remove the embedded-runner bridge only after the Cranelift path owns the same tests and emitted binaries no longer embed the interpreter payload

The first Cranelift landing should stay intentionally narrow:

- single-module programs
- top-level immutable bindings
- literals, identifier reads, binary operators, and block expressions
- unsupported constructs must continue to fail clearly and early

This keeps the initial backend milestone honest while still letting the repository start moving away from the Rust-source bridge.

## 15. CLI Plan

Initial commands:

```text
fscript run <file.fs>
fscript compile <input.fs> <output>
fscript check <file.fs>
fscript test
fscript fmt
```

v0.1 required commands:

- `run`
- `compile`
- `check`

`test` and `fmt` can land after the frontend is stable, but the plan should reserve room for them now.

### 15.1 CLI contract

The CLI contract should be stable early.

- `check` returns exit code `0` on success and non-zero on diagnostics.
- `run` returns the program exit code once runtime execution exists.
- `compile` returns non-zero on frontend, codegen, or link failures.
- diagnostics default to human-readable text output
- machine-readable output can be added later, but should not complicate the first slice
- input paths should be canonicalized through the same driver path rules used by module resolution
- docs and examples should distinguish between the Cargo package name `fscript-cli` and the installed/built binary name `fscript` so commands like `cargo run -p fscript-cli -- run file.fs` are easy to discover

## 16. Diagnostics Plan

Compiler UX matters a lot for a new language.

Use `miette` to provide:

- source-span highlights
- primary and secondary labels
- actionable messages
- notes and help text
- file-aware rendering

We should snapshot-test diagnostics from the beginning.

Diagnostic categories:

- lexical errors
- parse errors
- name resolution errors
- type errors
- effect violations
- runtime host-boundary errors
- compile/link errors

### 16.1 Diagnostic conventions

We should standardize:

- stable diagnostic IDs by subsystem
- file, line, and column reporting derived from shared source maps
- one primary label per main span, with optional secondary labels for related spans
- notes and help messages for actionable recovery where possible

## 17. Testing Strategy

The long-term project requirement is **100% test coverage on everything**.

We should enforce this as architecture, not aspiration, while still keeping early milestones practical.

### 17.1 Coverage policy

Coverage should be enforced in stages:

- each newly implemented crate should target 100% line coverage before the next major feature wave expands it
- the workspace-level coverage gate becomes mandatory once the interpreter slice is stable
- branch coverage should be enforced where the tooling can measure it reliably
- examples must be exercised by automated tests
- CLI commands must be exercised by automated tests

Use `cargo-llvm-cov` as the coverage gate.

### 17.2 Test layers

Every feature should have all of the following where appropriate:

- unit tests for local logic
- integration tests for crate boundaries
- snapshot tests for diagnostics and CLI output
- property tests for parser, typechecker, and IR invariants
- end-to-end tests that run real `.fs` programs
- compile tests that assert expected failures
- coverage tests that ensure examples are executed

### 17.3 Required test coverage by subsystem

`fscript-lexer`
: token streams, spans, invalid tokens, comments, trivia.

`fscript-parser`
: precedence, recovery, all syntax forms, malformed syntax.

`fscript-types`
: valid types, invalid types, inference, generics, unions, block values.

`fscript-effects`
: eager start, deferred behavior, purity violations, ordering guarantees.

`fscript-interpreter`
: semantics of every IR node.

`fscript-runtime`
: scheduler ordering, generator stepping, intrinsic behavior.

`fscript-codegen-cranelift`
: executable generation and parity with interpreter output.

`fscript-cli`
: command parsing, success output, failure output, exit codes.

### 17.4 Golden and property testing

Use `insta` for:

- syntax trees
- type errors
- effect diagnostics
- CLI help output
- formatted examples of emitted IR

Use `proptest` for:

- lexer never panics on arbitrary input
- parser never panics on arbitrary token streams
- parser round-trips on valid syntax subsets where applicable
- interpreter and native compiled output agree for generated programs in a restricted safe subset
- immutable operations do not mutate previous values

### 17.5 Current hardening focus

The repository is now in a coverage-driven hardening phase.

The current priority order should be:

1. raise workspace coverage from the low-70s into the 90s by targeting the largest remaining crates first
2. prefer direct unit and integration tests for currently unexecuted semantic paths over adding more broad smoke tests
3. keep parity tests growing between `run` and `compile` whenever a new backend/runtime slice lands
4. use coverage deltas to decide what to test next instead of expanding scope speculatively

As of the latest successful `cargo llvm-cov --workspace --all-features` run on March 8, 2026, the biggest remaining direct-coverage gaps are concentrated in:

- `fscript-driver`
- `fscript-std`
- `fscript-types`
- `fscript-interpreter`
- `fscript-codegen-cranelift`
- `fscript-compile-runner`

That distribution matters because it means the remaining work is mostly semantic-path and runtime-path hardening, not foundational frontend uncertainty.

## 18. Examples Plan

The repository must include an `examples/` folder with at least 10 runnable example apps.

Each example should:

- have a short README or header comment
- be runnable via `fscript run`
- be covered by automated tests
- demonstrate one or more language features clearly

Minimum example set:

1. `hello_world.fs`
2. `array_pipeline.fs`
3. `object_merge.fs`
4. `result_error_handling.fs`
5. `match_tagged_union.fs`
6. `generator_counter.fs`
7. `defer_lazy_work.fs`
8. `filesystem_read_write.fs`
9. `json_load_users.fs`
10. `notes_cli/main.fs`

Good optional additions:

11. `word_count.fs`
12. `static_site_snippet.fs`
13. `csv_transform.fs`
14. `mini_http_client.fs`
15. `task_parallel_fetch.fs`

### 18.1 Featured project examples

The repository should not stop at tiny single-file examples.

We should also maintain three more detailed example projects that prove FScript can scale from syntax demos into small real programs with multiple files, typed boundaries, and user-facing workflows.

These examples should be treated as product-grade fixtures:

- each project should live in its own folder under `examples/`
- each project should include multiple `.fs` modules, not just one file
- each project should be runnable with `fscript run`
- each project should become part of the end-to-end regression suite
- each project should have a short README explaining its commands, files, and language features
- each project should prefer the real `std:` modules over fake helpers so they exercise the runtime honestly

### 18.2 Project 1: `http_hello_server`

Purpose:

- prove that FScript can express a tiny network service, not just batch scripts
- provide the canonical example for request routing, response construction, and effectful host boundaries
- act as the first motivating consumer for a future `std:http` module

This project is intentionally small in behavior but deep in shape.
It should show a realistic module split, explicit data types, and a long-running runtime entrypoint.

Suggested file layout:

```text
examples/http_hello_server/
  README.md
  main.fs
  server.fs
  router.fs
  response.fs
  types.fs
```

Suggested behavior:

- start an HTTP server on a configurable port
- respond to `GET /` with plain text `hello from fscript`
- respond to `GET /health` with a small JSON body such as `{ tag: 'ok' }`
- respond to `GET /hello/:name` with `hello <name>`
- return a typed not-found response for unknown routes
- log startup information and per-request summaries through the runtime boundary

Suggested type shapes:

```fs
export type Request = {
  method: String,
  path: String,
  query: Unknown,
}

export type Response = {
  status: Number,
  headers: { contentType: String },
  body: String,
}

export type Route =
  | { tag: 'home' }
  | { tag: 'health' }
  | { tag: 'hello', name: String }
  | { tag: 'not_found' }
```

Language and runtime features exercised:

- module imports and exports
- explicit exported types
- tagged unions plus exhaustive `match`
- string helpers for path handling
- record construction and immutable updates
- effectful runtime calls
- long-running process semantics

Implementation notes:

- `http_hello_server` should be documented as depending on a minimal `std:http` module that lands after the current required v0.1 core modules
- the first version of `std:http` only needs `Http.serve` plus request/response records; it does not need a full client/server ecosystem
- this example is the clearest justification for adding `std:http` carefully rather than inventing network semantics ad hoc

Acceptance criteria:

- `fscript run examples/http_hello_server/main.fs` starts a local server
- repository docs should also show the Cargo invocation `cargo run -p fscript-cli -- run examples/http_hello_server/main.fs`
- requests to `/`, `/health`, and `/hello/world` return the expected values
- unknown routes produce a stable not-found response
- route handling is implemented through typed pattern matching, not stringly nested conditionals everywhere
- the example is covered by an integration test that boots the server, performs requests, and shuts it down cleanly

### 18.3 Project 2: `notes_cli`

Purpose:

- provide the canonical multi-command CLI example for FScript
- exercise the existing Draft 0.1 standard library more directly than the HTTP project
- prove that files, JSON boundaries, `Result`, and immutable updates compose well in real code

Suggested file layout:

```text
examples/notes_cli/
  README.md
  main.fs
  cli.fs
  notes.fs
  storage.fs
  format.fs
  types.fs
```

Suggested user commands:

- `notes add "buy milk"`
- `notes list`
- `notes show <id>`
- `notes done <id>`
- `notes delete <id>`

Suggested stored data shape:

```fs
export type Note = {
  id: String,
  body: String,
  done: Boolean,
  createdAt: String,
}

export type NoteStore = {
  notes: Note[],
}

export type NotesError =
  | { tag: 'invalid_command', message: String }
  | { tag: 'not_found', id: String }
  | { tag: 'decode_error', message: String }
```

Suggested behavior:

- persist notes in a JSON file inside the example directory
- create the store file automatically when it does not exist
- decode JSON explicitly at the boundary
- keep core note operations pure where possible
- surface expected failures with typed `Result<T, NotesError>` values
- print friendly text output for list and show commands

Language and runtime features exercised:

- user-module imports across a small project
- arrays, records, and immutable updates
- `std:filesystem` for persistence
- `std:json` for `jsonToObject` / `jsonToString` boundaries, including comment-tolerant config files
- `std:logger` for operator-facing terminal output and pretty JSON inspection
- `std:result` for recoverable errors
- pipe syntax for list formatting and transformations
- explicit exported return types on public helpers

Why this project matters:

- it maps closely to how many users evaluate a new language: can I build a small tool that reads data, transforms it, and writes it back safely?
- it is realistic without requiring network support
- it should become the main regression fixture for `std:filesystem`, `std:json`, and `std:logger`

Acceptance criteria:

- each command works through `fscript run`
- repeated runs preserve state on disk
- malformed JSON in the store file produces a typed, user-facing error path
- adding or marking a note done rewrites the store immutably rather than mutating shared state
- the example has end-to-end tests covering success cases and expected failures

### 18.4 Project 3: `static_site_builder`

Purpose:

- provide a more “project-shaped” batch program than the notes CLI
- demonstrate directory traversal, file transforms, and multi-step pipelines
- give the runtime and compiler a realistic workload with many files and predictable output

Suggested file layout:

```text
examples/static_site_builder/
  README.md
  main.fs
  discover.fs
  parse.fs
  render.fs
  write.fs
  types.fs
  sample-content/
  output/
```

Suggested behavior:

- read a content directory of text or lightweight frontmatter-like source files
- parse each file into a typed `Page`
- generate a small HTML page for each entry
- generate an `index.html` page that links to all generated pages
- write the built files into an output directory

Suggested type shapes:

```fs
export type Page = {
  slug: String,
  title: String,
  body: String,
}

export type BuildError =
  | { tag: 'invalid_page', path: String, message: String }
  | { tag: 'write_failed', path: String, message: String }
```

Language and runtime features exercised:

- directory-oriented `std:filesystem` usage
- `std:string` parsing and joining helpers
- `std:array` pipelines for sorting and rendering collections
- `Result`-based validation
- multi-module pure/effectful boundaries
- deterministic end-to-end output suitable for snapshot tests

Why this is the right third project:

- it stays inside the currently planned v0.1 stdlib surface
- it is meaningfully different from the notes CLI
- it doubles as a high-value compile target because it performs enough real work to flush out IR and runtime issues

Acceptance criteria:

- running the project builds a complete output directory from sample input files
- generated HTML is stable enough for snapshot testing
- invalid input files fail with typed diagnostics rather than generic crashes
- pure parsing and rendering code remain isolated from effectful file IO modules

### 18.5 Delivery order for featured projects

These three projects should land in this order:

1. `notes_cli`
2. `static_site_builder`
3. `http_hello_server`

Rationale:

- `notes_cli` depends only on already-planned core std modules and should arrive first
- `static_site_builder` deepens the same runtime surface with richer file workflows
- `http_hello_server` should land once the repository is ready to add a minimal `std:http` without distracting from the v0.1 filesystem/json/runtime milestones

## 19. Performance Plan

We should not optimize too early, but we should structure the code so optimization is possible.

Performance work should focus on:

- zero-copy or low-copy lexing/parsing where practical
- compact span storage
- fixed-layout runtime values where practical
- avoiding scheduler overhead for pure paths
- reducing closure allocations from currying where possible
- efficient immutable collections
- interpreter vs native parity benchmarking

Recommended benchmarking:

- parser throughput benchmark
- typechecker benchmark
- interpreter benchmark
- compile throughput benchmark
- runtime benchmark for array pipelines and filesystem-heavy examples

## 20. Development Milestones

### Milestone 1: workspace bootstrap

Deliver:

- Cargo workspace
- Rust 2024 edition setup
- `fscript` CLI skeleton
- source/span infrastructure
- test-support scaffolding

### Milestone 2: lexical frontend

Deliver:

- token model
- lexer
- lexical diagnostics
- snapshot tests for lexing

### Milestone 3: parseable frontend

Deliver:

- parser
- AST
- syntax diagnostics and recovery
- `fscript check`

### Milestone 4: full parseable frontend

Deliver:

- expanded AST for declarations, patterns, types, and expressions
- expanded parser for the Draft 0.1 surface syntax
- syntax diagnostics and recovery for declarations, expressions, and patterns
- `fscript check` validating both lexing and parsing
- all current examples parsing successfully

### Milestone 4.5: bootstrap executable data subset

Deliver:

- bootstrap `run` support for plain-data execution beyond literals
- default `std:` imports for the first runtime-backed modules needed by examples
- immutable record and array runtime values
- native-curried stdlib calls for the supported bridge modules
- `examples/object_merge.fs` passing under `fscript run`
- clear documentation of the remaining gap between bootstrap `run` and final interpreter goals

### Milestone 4.6: expanded bridge example slice

Deliver:

- user-defined curried functions, pipes, `if`, `match`, and pure generators executing in the bridge
- bridge-backed `std:array`, `std:string`, `std:number`, and `std:result`
- every currently shipped example passing under `fscript run`
- example-backed regression tests for the shipped bridge-supported examples

### Milestone 5: semantic frontend

Deliver:

- module resolution
- name resolution
- HIR
- typechecker
- effect analyzer
- diagnostic coverage

### Milestone 6: interpreter

Deliver:

- IR
- runtime core
- interpreter
- `fscript run`
- first 10 examples passing

### Milestone 7: standard library

Deliver:

- `std:array`
- `std:object`
- `std:string`
- `std:number`
- `std:result`
- `std:json`
- `std:filesystem`
- tests and examples for each

### Milestone 8: native compilation

Deliver:

- a documented native runtime ABI shared by `fscript-runtime` and `fscript-codegen-cranelift`
- Cranelift lowering for the current pure subset plus strings, aggregates, control flow, and structural equality
- native lowering for user-defined functions, currying, and closures
- native generator frames and resume logic
- native deferred/task/effect lowering against the shared scheduler/runtime contract
- native module-graph initialization and stdlib import wiring
- backend-owned parity coverage for every supported example
- removal of the embedded-runner default path from ordinary successful `fscript compile`

### Milestone 9: hardening

Deliver:

- workspace coverage gate enforced in CI
- property tests expanded
- benchmark suite
- performance tuning
- improved diagnostics

### Milestone 9.1: final coverage campaign

Deliver:

- targeted tests for the remaining uncovered paths in `fscript-driver`
- targeted stdlib tests for error and edge paths in `fscript-std`
- targeted type-system tests for currently uncovered narrowing, compatibility, and diagnostic paths in `fscript-types`
- interpreter and scheduler tests for remaining uncovered forcing and effect-ordering paths
- compile-runner and codegen tests for invalid image, fallback, and backend error branches
- updated coverage notes in this plan after each substantial campaign

## 21. Risks and Mitigations

### Risk: implicit async semantics become expensive

Mitigation:

- keep pure/effectful separation explicit in IR
- lower only effectful paths into scheduler state machines
- benchmark effect-heavy examples early

### Risk: default currying causes allocation overhead

Mitigation:

- represent curried functions efficiently in HIR and IR
- specialize fully applied calls
- add benchmarks before optimizing blindly

### Risk: immutable updates become too copy-heavy

Mitigation:

- start simple
- profile
- move to persistent or copy-on-write representations where justified

### Risk: codegen diverges from interpreter behavior

Mitigation:

- make IR the single semantic contract
- run parity tests for interpreter vs compiled executable on all examples

### Risk: diagnostics become inconsistent across crates

Mitigation:

- share span and diagnostic conventions early
- snapshot-test representative errors for every subsystem
- avoid ad hoc CLI-only formatting logic

### Risk: 100% coverage goal becomes performative

Mitigation:

- require meaningful tests, not just line execution
- pair coverage gates with snapshot, property, and end-to-end tests
- treat untestable design as a design smell

## 22. Definition of Done for v0.1

FScript v0.1 is done when all of the following are true:

- `fscript run` works for the language subset described by `specs/LANGUAGE.md`, `specs/GRAMMAR.md`, `specs/TYPESYSTEM.md`, `specs/RUNTIME.md`, and `specs/STDLIB.md`
- `fscript compile` emits native executables for the same supported subset without embedding the interpreter-backed compile runner
- ill-typed programs are rejected before execution
- examples folder contains at least 10 runnable apps
- every example is covered by automated tests
- interpreter and compiled output agree on supported examples
- workspace coverage gate is 100%
- diagnostics are snapshot-tested
- standard-library core modules are implemented and tested
- Rust implementation follows `CODE_STYLE.md`

## 23. AI Agent Build Checklist

Use this as the task list for an implementation agent.

Status note: checkboxes below are intentionally conservative. Items are ticked only when the repository already contains a real implementation, not just scaffolding or a future placeholder.

Additional status note: representative lexer/parser output, driver diagnostics, and CLI output now have committed snapshot coverage; the CLI has end-to-end tests for successful `run`/`compile` flows plus expected compile failures; and lexer/parser crates now carry `proptest` coverage for span ordering, identifier-like tokenization, generated binding modules, and top-level trivia stability.

Additional backend note: `fscript-codegen-cranelift` now contains a first bounded real Cranelift path for the numeric single-module subset, and the older generated Rust-source compiler backend has been replaced by a fixed embedded-runner bridge for the broader subset. The Cranelift checklist items below are ticked for that shipped native slice, while the remaining backend work is to keep shrinking the interpreter-backed fallback until the real native path owns the full supported compile surface.

Additional coverage note: the last successful full `cargo llvm-cov --workspace --all-features` run on March 8, 2026 reported 73.14% workspace region coverage and 75.06% workspace line coverage. `fscript-ast`, `fscript-hir`, `fscript-ir`, `fscript-lexer`, `fscript-runtime`, `fscript-source`, and `fscript-test-support` now reach 100% direct line coverage under that command; `fscript-cli`, `fscript-effects`, and `fscript-wasm` are close behind; and the final workspace-coverage checkbox remains intentionally unticked while the remaining gap stays concentrated primarily in `fscript-driver`, `fscript-std`, `fscript-types`, `fscript-interpreter`, `fscript-codegen-cranelift`, and `fscript-compile-runner`.

- [x] Create a Cargo workspace using Rust 2024 edition.
- [x] Add workspace crates: `fscript-cli`, `fscript-source`, `fscript-lexer`, `fscript-parser`, `fscript-ast`, `fscript-hir`, `fscript-types`, `fscript-effects`, `fscript-lower`, `fscript-ir`, `fscript-runtime`, `fscript-interpreter`, `fscript-codegen-cranelift`, `fscript-driver`, `fscript-std`, `fscript-test-support`.
- [x] Wire `clap` into `fscript-cli` with `run`, `compile`, and `check` subcommands.
- [x] Implement source file, span, and diagnostic file-ID infrastructure.
- [x] Implement a working lexer for the current frontend slice.
- [x] Keep the handwritten lexer for Draft 0.1; revisit `logos` only if maintenance or performance data justifies it after the frontend freezes.
- [x] Implement parser error recovery and AST generation for the initial literal-and-identifier subset.
- [x] Expand AST generation to cover the Draft 0.1 surface syntax.
- [x] Expand the parser to cover imports, exports, types, functions, blocks, records, arrays, calls, pipes, `if`, `match`, `try/catch`, `throw`, `yield`, and generator arrows.
- [x] Make `fscript check` fail on parse errors, not just lex errors.
- [x] Add snapshot tests for lexing and parsing using `insta`.
- [x] Expand the bootstrap evaluator in `fscript-driver` to support the plain-data bridge subset used by `examples/object_merge.fs`.
- [x] Add runtime-backed `std:` module loading for the bootstrap evaluator, starting with `std:object`.
- [x] Extend the bootstrap evaluator to execute user-defined functions and lexical closures.
- [x] Extend the bootstrap evaluator to execute pipe expressions through data-last argument insertion.
- [x] Extend the bootstrap evaluator to execute `if` and `match` expressions with pattern bindings.
- [x] Extend the bootstrap evaluator to materialize pure generators as bridge sequence values.
- [x] Add bridge runtime support for `std:array`.
- [x] Add bridge runtime support for `std:string`.
- [x] Add bridge runtime support for `std:number`.
- [x] Add bridge runtime support for `std:result`.
- [x] Implement HIR lowering from AST.
- [x] Implement name resolution.
- [x] Implement the typechecker for primitives, records, arrays, functions, tagged unions, generics, and block values.
- [x] Add `match` exhaustiveness checking for tagged unions.
- [x] Enforce the spec rule that value-position `if` expressions require `else`.
- [x] Implement the effect analyzer for pure, eager effectful, and deferred effectful code.
- [x] Extend the semantic frontend to typecheck bootstrap `try/catch`, `throw`, and `defer`.
- [x] Extend the bootstrap evaluator to execute `try/catch` and `throw`.
- [x] Extend the bootstrap evaluator to execute lazy memoized `defer` values at consumption sites.
- [x] Lower pipes, currying, destructuring, `match`, and generators into HIR/IR.
- [x] Design and implement the IR.
- [x] Implement the native Rust runtime and single-threaded scheduler.
- [x] Implement generator runtime support.
- [x] Implement `defer` semantics in the IR and runtime.
- [x] Implement `std:array`.
- [x] Implement `std:object`.
- [x] Implement `std:string`.
- [x] Implement `std:number`.
- [x] Implement `std:result`.
- [x] Implement `std:json`.
- [x] Expand `std:json` with `jsonToObject`, `jsonToString`, `jsonToPrettyString`, and relaxed comment-tolerant parsing.
- [x] Implement `std:logger`.
- [x] Implement `std:filesystem`.
- [x] Implement the current shared-runtime `std:task` subset (`Task.all`, `Task.defer`, `Task.force`).
- [x] Expand the shared-runtime `std:task` surface with `Task.race` and `Task.spawn`.
- [x] Preserve deferred/task handles across `std:task` native calls instead of forcing task-native arguments before dispatch.
- [x] Move deferred/task forcing into a shared single-threaded runtime scheduler used by the interpreter.
- [x] Route ordinary effectful native stdlib calls through shared runtime deferred handles and scheduler execution.
- [x] Make ordinary effectful calls start eagerly at reach-time, not only when later forced.
- [x] Build the IR interpreter and make `fscript run` execute real programs.
- [x] Create `examples/` and add at least 10 runnable example apps.
- [x] Add automated tests that execute every example currently shipped in `examples/`.
- [x] Implement a bootstrap Rust-source compiler backend that emits native executables for the currently supported subset.
- [x] Expand the bootstrap compiler backend to support plain-data/control-flow IR forms: records, arrays, non-`defer` unary operators, `if`, member access, and index access.
- [x] Add bootstrap-compiler parity coverage for the plain-data/control-flow subset.
- [x] Expand the bootstrap compiler to consume the shared IR program graph so compiled executables can include user-module imports instead of compiling only a single module.
- [x] Add an interpreter-backed bootstrap native bridge so compiled executables can reuse the shared IR/runtime semantics for the broader currently supported subset while Cranelift is still pending.
- [x] Implement Cranelift lowering from FScript IR.
- [x] Emit native object files with `cranelift-module` and `cranelift-object`.
- [x] Link emitted objects with the runtime to produce executables for `fscript compile`.
- [x] Remove the bootstrap Rust-source compiler backend once the IR plus Cranelift pipeline is ready.
- [x] Add interpreter vs compiled parity tests.
- [x] Define and document the stable native ABI between generated code and `fscript-runtime`.
- [x] Choose and implement the native value representation strategy for primitives, strings, arrays, records, closures, generators, deferred handles, and task handles.
- [ ] Expand native lowering beyond numeric-only code to cover strings, booleans, null, undefined, records, arrays, member access, index access, and structural equality.
  - [x] Lower the first handle-backed native slice for strings, booleans, records, block expressions, and imported std member calls needed by `examples/filesystem.fs`.
- [ ] Lower `if`, `match`, and destructuring through the native path with parity against the interpreter.
  - [x] Lower `if` expressions through the native handle-backed slice for the current filesystem example parity target.
- [ ] Lower user-defined functions, currying, partial application, and closures without interpreter participation.
- [ ] Lower user-module imports/exports and once-per-module initialization entirely through the native path.
- [ ] Lower pure generators to native generator frames with explicit resume state and parity tests.
- [ ] Lower native `defer` and eager effect start through runtime-managed deferred/task handles instead of the embedded interpreter bridge.
  - [x] Lower deferred `FileSystem.readFile` handles and eager filesystem host calls through native runtime shims for `examples/filesystem.fs`.
- [ ] Add native scheduler parity coverage for dependency ordering, `Task.force`, `Task.all`, `Task.race`, and `Task.spawn`.
- [ ] Move pure stdlib helpers onto native lowering or explicit runtime shims:
  - [ ] `std:array`
  - [ ] `std:object`
  - [ ] `std:string`
  - [ ] `std:number`
  - [ ] `std:result`
- [ ] Move host-boundary stdlib helpers onto native runtime shims with parity coverage:
  - [ ] `std:json`
  - [ ] `std:logger`
  - [ ] `std:filesystem`
  - [ ] `std:task`
  - [x] Route `std:json` `jsonToPrettyString` through a native runtime call path.
  - [x] Route `std:filesystem` `readFile`, `writeFile`, `exists`, and `deleteFile` through native runtime call paths.
- [x] Add a backend parity table for every stdlib export and keep it updated until all supported exports are native-owned.
- [ ] Make CI fail when examples expected to be native-owned route through the embedded-runner fallback.
  - [x] Add a regression test asserting `examples/filesystem.fs` compiles without embedding the compile-runner program image.
- [ ] Remove the embedded-runner bridge from successful default `fscript compile` output once native parity owns the supported surface.
- [x] Add property tests with `proptest` for lexer, parser, and semantic invariants.
- [x] Add snapshot tests for diagnostics and CLI output.
- [x] Stabilize compile-error snapshot normalization across backend temp-directory naming so diagnostics and coverage snapshots stay deterministic.
- [x] Add end-to-end tests for successful runs and expected compile failures.
- [x] Make `cargo clippy --all-targets --all-features -- -D warnings` pass.
- [x] Add `cargo-llvm-cov` coverage gating in CI.
- [ ] Reach 100% coverage across the workspace.
  - [x] Bring `fscript-ast` to 100% direct crate coverage.
  - [x] Bring `fscript-hir` to 100% direct crate coverage.
  - [x] Bring `fscript-ir` to 100% direct crate coverage.
  - [x] Bring `fscript-lexer` to 100% direct crate line coverage.
  - [x] Bring `fscript-runtime` to 100% direct crate line coverage.
  - [x] Add host-side unit coverage for the `fscript-wasm` sandbox adapter result-shaping path.
  - [x] Continue hardening `fscript-test-support` toward full coverage.
  - [ ] Drive `fscript-driver` coverage up by targeting uncovered diagnostic, module-loading, compile, and runtime error branches.
  - [ ] Drive `fscript-std` coverage up by targeting remaining edge-case and failure-path behavior in runtime-backed modules.
  - [ ] Drive `fscript-types` coverage up by targeting uncovered narrowing, compatibility, and diagnostic paths.
  - [ ] Drive `fscript-interpreter` coverage up by targeting remaining effect, forcing, and control-flow edge cases.
  - [ ] Drive `fscript-codegen-cranelift` and `fscript-compile-runner` coverage up by targeting backend fallback and invalid-image/error branches.
- [x] Document how to run, compile, test, and measure coverage.
