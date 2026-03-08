# Rust Code Style Guide

Status: Draft 0.1

## 1. Purpose

This document defines how we write Rust in the FScript codebase.

The goal is not to be clever or unique.
The goal is to follow mainstream Rust community conventions closely enough that a typical experienced Rust developer can move through the codebase comfortably.

Project priorities:

- consistency over personal taste
- boring, readable code over clever code
- default community tooling over custom formatting rules
- small APIs with clear ownership
- strong tests and diagnostics

## 2. Source of Truth

When this file is silent, follow the mainstream Rust toolchain and official guidance in this order:

1. `rustfmt` stable defaults
2. Clippy guidance
3. official Rust Style Guide
4. Rust API Guidelines
5. standard library conventions

Do not invent a project-specific style rule unless it clearly improves this codebase.

## 3. Edition and Toolchain

Use:

- stable Rust
- Rust 2024 edition
- `rustfmt`
- `clippy`

Rules:

- code must compile on stable Rust
- all code must be formatted with `cargo fmt`
- all code must pass `cargo clippy`
- warnings should be treated as errors in CI unless explicitly justified

## 4. Formatting

Use default `rustfmt` formatting.

Rules:

- do not hand-format to fight `rustfmt`
- do not add a large custom `rustfmt.toml` unless we have a compelling repo-wide need
- prefer formatting decisions that stay stable under `cargo fmt`
- long chains and builders should be formatted the way `rustfmt` wants, not manually aligned for style points

## 5. Naming

Follow standard Rust naming conventions.

Use:

- `snake_case` for functions, methods, variables, modules, and files
- `PascalCase` for structs, enums, traits, and type aliases
- `SCREAMING_SNAKE_CASE` for constants and statics
- concise names for tight local scope
- descriptive names for public APIs and semantic compiler phases

Examples:

- `parse_module`
- `lower_pipe_expr`
- `TypeChecker`
- `EffectKind`
- `MAX_ARITY`

Avoid:

- abbreviations that are not already standard in Rust/compiler work
- cute names
- single-letter names outside tiny local algorithmic contexts

## 6. Modules and Files

Organize code by responsibility, not by arbitrary technical layers inside a single file.

Rules:

- keep modules focused
- prefer several small modules over one giant file
- match filenames to module purpose
- avoid deeply nested module hierarchies unless they provide real clarity
- put tests near the code when they are unit tests; use integration tests for cross-crate behavior

Good:

- `parser.rs`
- `parser/expr.rs`
- `effects.rs`
- `runtime/task.rs`

Less good:

- `misc.rs`
- `helpers.rs`
- `utils.rs` unless the helpers are genuinely cohesive

## 7. Imports

Keep imports simple and unsurprising.

Rules:

- group standard library imports before external crate imports before local crate imports
- prefer explicit imports over wildcards
- avoid `use super::*`
- import the narrowest useful path that improves readability
- if a type or function is used once and the full path is clearer, using the full path is fine

Example style:

```rust
use std::path::PathBuf;

use miette::{IntoDiagnostic, Result};

use crate::ast::Expr;
use crate::span::Span;
```

## 8. Functions and Methods

Write small functions with one clear responsibility.

Rules:

- prefer free functions unless a method materially improves discoverability or invariants
- keep public functions straightforward and predictable
- return early on error conditions
- avoid deeply nested control flow
- do not hide important work in surprising trait impls or macros

Prefer:

- clear argument names
- small helper functions when they improve readability
- explicit return types on public functions

## 9. Structs and Enums

Prefer plain structs and enums with obvious ownership and invariants.

Rules:

- make fields private unless public visibility adds real value
- prefer constructors or smart constructors when invariants matter
- use enums for closed sets of variants
- derive standard traits where appropriate: `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`
- do not derive traits blindly if they create incorrect semantics or expensive accidental clones

## 10. Traits

Use traits when they model a stable behavioral abstraction, not just to avoid writing a match statement.

Rules:

- prefer concrete types first
- introduce traits when there are multiple real implementations or a strong abstraction boundary
- keep traits small and cohesive
- avoid trait-heavy designs that make ownership and control flow harder to follow
- be cautious with blanket impls and complex generic trait constraints in early compiler/runtime code

## 11. Ownership and Borrowing

Lean into standard Rust ownership patterns instead of fighting them.

Rules:

- prefer borrowing for read-only access
- take ownership when the function logically consumes the value
- clone intentionally, not accidentally
- if cloning is cheap and clarifies code, it is acceptable
- if cloning is expensive or repeated, measure and improve it

Do not contort APIs purely to avoid every clone at all costs.

## 12. Error Handling

Prefer explicit error handling with `Result`.

Rules:

- use `Result<T, E>` for fallible operations
- use `thiserror` for internal error types when it improves clarity
- use `miette` or compatible diagnostics at reporting boundaries
- do not use `panic!` for ordinary control flow
- do not use `unwrap()` or `expect()` in production code unless the invariant is truly impossible to violate and the message is excellent
- `unwrap()` and `expect()` are acceptable in tests when they improve readability

Compiler rule of thumb:

- internal logic errors should be rare and obvious
- user-facing mistakes should become diagnostics, not panics

## 13. Documentation

Document public APIs and non-obvious invariants.

Rules:

- public items should have doc comments unless the meaning is completely obvious and local
- modules should have top-level docs when they define an important subsystem
- explain why for tricky code, not just what
- keep doc comments factual and concise
- include examples when that materially helps usage

For compiler internals, document:

- phase boundaries
- ownership expectations
- invariants
- desugaring assumptions
- performance-sensitive behaviors when relevant

## 14. Comments

Comments should explain intent, invariants, or tradeoffs.

Avoid comments that merely restate the code.

Good comment topics:

- why this branch exists
- why this data structure is shaped this way
- why this operation must preserve source ordering
- why a clone is intentional

## 15. Pattern Matching

Use `match` when it improves clarity, especially for enums.

Rules:

- prefer exhaustive `match` for semantic enums
- avoid `_` catch-all arms when enumerating variants would be clearer and safer
- use `if let` and `while let` when there is exactly one interesting pattern
- keep match arms short where practical

## 16. Iteration and Collections

Use iterator adapters when they are readable; use loops when they are clearer.

Rules:

- do not force iterator chains when a loop is easier to understand
- do not force loops when iterator methods communicate the intent better
- prefer clarity over ideology
- be mindful of allocation behavior in hot paths

## 17. Macros

Use macros conservatively.

Rules:

- prefer functions, traits, and types first
- use derive macros and mainstream helper macros freely when they improve code quality
- avoid custom declarative or procedural macros unless the payoff is substantial
- do not hide control flow or ownership in surprising macros

## 18. Generics

Use generics where they improve APIs or reuse, but avoid needless generic complexity.

Rules:

- prefer concrete types in internal code until generality is actually needed
- keep generic bounds readable
- use `where` clauses for complex bounds
- avoid type-level cleverness that hurts maintainability

## 19. Lifetimes

Let lifetime elision work where it can.

Rules:

- write explicit lifetimes when they clarify relationships or are required
- avoid over-annotating lifetimes just to be explicit
- design APIs that are straightforward to use with ordinary borrowing patterns

## 20. Unsafe Code

Unsafe code is allowed only when necessary and justified.

Rules:

- prefer safe Rust
- every unsafe block must have a clear safety comment explaining the invariant being relied on
- keep unsafe blocks as small as possible
- wrap unsafe details behind safe APIs
- add focused tests around unsafe code

No unsafe code should land just because it seems faster.
Profile first.

## 21. Testing Style

Testing is part of the design, not a cleanup step.

Rules:

- write unit tests close to the implementation
- write integration tests for crate boundaries and CLI behavior
- use snapshot tests for diagnostics and formatted output
- use property tests for parser and semantic invariants
- test names should describe behavior, not implementation trivia
- keep test setup simple and explicit

Because this project requires 100% coverage:

- every branch should exist for a reason
- every branch should have a test
- untestable code is a design smell

## 22. Lints

Default posture:

- fix the lint
- do not silence the lint reflexively

Rules:

- if a lint is noisy but correct, improve the code
- if a lint must be suppressed, scope the suppression narrowly
- every non-obvious `#[allow(...)]` should have a short reason comment
- do not add crate-wide `allow` attributes casually

## 23. Public API Style

For public crate APIs, follow standard Rust API expectations.

Rules:

- use conventional names
- keep constructors and conversions unsurprising
- prefer explicit types and ownership semantics
- avoid surprising side effects
- keep error types meaningful
- keep APIs easy to discover from docs and IDE completion

## 24. Performance Style

Do not write prematurely optimized code that is hard to maintain.

Rules:

- establish correctness first
- profile before optimizing
- preserve obvious code unless a measured hotspot demands change
- document non-obvious performance tradeoffs
- optimize data layout and allocation behavior where it matters, not everywhere

## 25. Example Defaults

Good everyday defaults for this repository:

- use `cargo fmt`
- use `cargo clippy --all-targets --all-features`
- prefer `Result` over panic
- prefer explicit small enums over dynamic trait objects
- prefer focused modules over giant files
- prefer simple control flow over abstraction for its own sake

## 26. References

This guide is intentionally aligned with mainstream Rust guidance, especially:

- Rust Style Guide
- Rust API Guidelines
- The Rust Programming Language
- rustfmt defaults
- Clippy guidance
- rustdoc conventions
