# FScript Type System Specification

Status: Draft 0.1

## 1. Goal

Define a strict, sound, mostly inferred type system for FScript that feels familiar to TypeScript users while giving the compiler much stronger guarantees.

The type system should support:

- compile-time rejection of ill-typed programs
- no implicit `any`
- no runtime type errors inside well-typed FScript code
- structural typing for plain data
- immutable data semantics
- functional composition
- effect-aware typing without forcing `async` / `await` syntax

## 2. Design Principles

The FScript type system should be:

- strict
- structural
- predictable
- mostly inferred
- explicit at module boundaries
- sound enough to support ahead-of-time native compilation

Practical summary:

- local code should usually not need much type annotation
- exported APIs should still be clear and stable
- bad types should stop compilation immediately
- runtime checks should exist only at host/data boundaries

## 3. Soundness Goal

A well-typed FScript program should not fail because of an internal type mismatch at runtime.

This means:

- type errors are compile errors
- effectful code is still typechecked statically
- parsing, decoding, file input, JSON, and future host interop remain boundary points where values may need validation

FScript does not aim to preserve JavaScript's runtime flexibility.

## 4. Core Type Model

Draft 0.1 supports these type categories:

- primitive types
- record types
- array types
- function types
- generator sequence types
- tagged union types
- generic types
- literal types
- `Never`
- `Unknown`

Recommended canonical built-in type names:

- `Number`
- `String`
- `Boolean`
- `Null`
- `Undefined`
- `Never`
- `Unknown`

## 5. Type Inference

FScript should infer types for local bindings and local function results where possible.

Examples:

```fs
answer = 42
name = 'Ada'
add = (a: Number, b: Number) => a + b
```

Inference rules:

- local immutable bindings may omit type annotations when inference is unambiguous
- function parameters should usually be annotated in Draft 0.1
- return types may be inferred for local functions
- exported functions should have explicit return types in Draft 0.1
- recursive functions should require explicit annotations where inference would be ambiguous or unstable

Rationale:

- this keeps local code lightweight like TypeScript
- it keeps module APIs stable and readable
- it simplifies compiler implementation in the early language versions

## 6. Primitive Types

Primitive values map to primitive types.

```fs
count = 10          // Number
name = 'Ada'        // String
active = true       // Boolean
empty = Null        // Null
missing = Undefined // Undefined
```

Draft 0.1 should not implicitly coerce between primitive types.

Examples:

- `Number` is not automatically a `String`
- `Null` is not assignable to all types by default
- `Undefined` is not assignable to all types by default

## 7. Record Types

Records are structurally typed immutable data.

Example:

```fs
type User = {
  id: String,
  name: String,
  active: Boolean,
}
```

Rules:

- record compatibility is structural
- field names and field types must match
- field order does not matter
- record values are immutable
- field assignment is invalid

Draft 0.1 recommendation:

- record shapes should be closed by default
- unknown extra fields should not be silently accepted unless explicitly designed later

This helps soundness, optimization, and diagnostics.

## 8. Array Types

Arrays are homogeneous ordered immutable collections.

Example:

```fs
numbers = [1, 2, 3] // Number[]
```

Rules:

- array element type is inferred from elements
- mixed-type arrays require a union element type
- arrays are immutable
- index assignment is invalid
- array-transforming operations return new arrays

Examples:

```fs
values = [1, 'two'] // (Number | String)[]
```

## 9. Function Types

Functions are first-class and curried by default.

Examples:

```fs
add = (a: Number, b: Number): Number => a + b
```

Semantically:

```fs
add : Number -> Number -> Number
```

Rules:

- all multi-parameter functions are curried
- partial application is always valid when argument types line up
- calling with too many arguments is a type error
- function values are immutable
- functions are not structurally comparable with `===`

## 10. Generator Types

Generators produce lazy sequences.

Example:

```fs
numbers = *(): Sequence<Number> => {
  yield 1
  yield 2
  yield 3
}
```

Rules:

- a generator arrow has type `Sequence<T>`
- `yield expr` requires `expr` to have type `T`
- generator arrows are intended for pure lazy iteration in Draft 0.1
- yielding an effectful computation is a type/effect error in Draft 0.1

Async streams are not generators and should use a separate standard-library abstraction such as `Stream<T>` in future versions.

## 11. Tagged Unions

Tagged unions are the preferred sum type model.

Example:

```fs
type User =
  | { tag: 'guest' }
  | { tag: 'member', id: String, name: String }
```

Rules:

- variants are distinguished structurally
- a stable discriminant field such as `tag` is the recommended convention
- `match` should be exhaustive over tagged unions
- pattern matching narrows the type in each branch

## 12. Literal Types

Literal values may participate in narrower types.

Examples:

```fs
status = 'ok' // 'ok'
type Status = 'ok' | 'error'
```

Literal types are especially useful for:

- tagged unions
- small domain values
- match exhaustiveness

## 13. Union Types

Union types represent values that may be one of several alternatives.

Example:

```fs
type Id = Number | String
```

Rules:

- a value of a union type may be used only in ways valid for all members until narrowed
- `match` and pattern tests should narrow union types
- unions involving records should preserve structural matching rules

## 14. Intersection Types

Intersection types may exist in the surface language, but they should be kept conservative in Draft 0.1.

Example:

```fs
type Named = { name: String }
type Aged = { age: Number }
type Person = Named & Aged
```

Recommended implementation rule:

- support intersections where they can be resolved into compatible record/function constraints cleanly
- reject ambiguous or unsound intersections rather than inventing surprising rules

## 15. Generic Types

FScript should support parametric polymorphism.

Examples:

```fs
type Maybe<T> = T | Null
map = <T, U>(fn: T -> U, items: T[]): U[] => ...
```

Rules:

- type parameters are lexically scoped
- generic functions may infer type arguments when possible
- exported generic APIs should remain readable in diagnostics and docs

Recommended implementation strategy:

- begin with monomorphization-friendly generic semantics
- keep runtime representation simple where possible
- avoid JS-style erased-but-unsound behavior

## 16. Destructuring

Destructuring should participate in typechecking fully.

Examples:

```fs
{ name, age } = user
[first, second] = pair
```

Rules:

- object destructuring requires the corresponding record fields
- array destructuring requires compatible positional types
- destructuring patterns in parameters and `match` branches participate in type narrowing

## 17. Equality

`===` and `!==` are structural for plain data.

Rules:

- `Number`, `String`, `Boolean`, `Null`, and `Undefined` compare by value
- records compare structurally
- arrays compare structurally
- tagged unions compare structurally
- functions are not comparable
- generators and streams are not comparable

This is intentionally different from JavaScript reference identity.

## 18. Block Types

Blocks evaluate to the type of their final expression.

Example:

```fs
label = {
  name = 'Ada'
  name
}
```

Rules:

- earlier bindings inside a block influence later expressions
- the final expression determines the block result type
- if a block ends in `throw`, its type is `Never`

## 19. `if`, `match`, and `try/catch`

These are expressions and must typecheck as expressions.

### 19.1 `if`

Rules:

- condition must be `Boolean`
- when an `if` is used as a value, both branches must exist
- both branches must unify to a common type, or one branch may be `Never`

### 19.2 `match`

Rules:

- each branch result must unify to a common type, or a branch may be `Never`
- tagged unions should be checked for exhaustiveness
- non-exhaustive `match` over a tagged union is a compile error

### 19.3 `try/catch`

Rules:

- `try` and `catch` branches must unify to a common type, or one branch may be `Never`
- `catch` binds the thrown value with its inferred or declared shape
- `throw` has type `Never`

## 20. `Never` and Unreachable Code

`Never` represents an expression that does not produce a value.

Examples:

- `throw someError`
- future fatal runtime intrinsics

Rules:

- `Never` can unify upward where needed for branch typing
- code after an expression of type `Never` may be treated as unreachable

## 21. `Unknown`

`Unknown` is allowed as a boundary type for host values or undecoded data.

Examples:

- JSON before decoding
- future FFI values
- dynamically loaded host data

Rules:

- `Unknown` cannot be used freely as if it were another type
- it must be narrowed, decoded, or validated before ordinary use
- the standard library should expose explicit decoding and validation APIs

There is no implicit `any` in FScript.

## 22. Error Typing

Expected failures should be modeled with `Result<T, E>`.

Example:

```fs
type ParseError = {
  tag: 'parse_error',
  message: String,
}

parsePort = (text: String): Result<Number, ParseError> => ...
```

Rules:

- recoverable failures should prefer `Result<T, E>`
- `throw` is for exceptional failure, not ordinary control flow
- thrown values should usually be plain tagged records

## 23. Effects and Types

Effects are not ordinary value types, but the type system must cooperate with effect analysis.

Recommended rule for Draft 0.1:

- type inference and effect inference run together
- a function that calls an effectful function becomes effectful
- pure functions cannot secretly remain typed as pure if they perform effects

Surface syntax may omit explicit effect annotations in Draft 0.1, but the compiler should know and report them for exported functions.

## 24. Module Boundary Rules

To keep APIs readable and stable, Draft 0.1 should require:

- explicit return types on exported functions
- explicit type declarations for exported public constants when inference would be unclear
- typechecking across module imports before execution

This is a practical compromise between inference and maintainability.

## 25. Runtime Checks

Runtime type checks should be minimized.

Allowed/expected runtime checks:

- decoding external data
- validating `Unknown`
- host/runtime boundary assertions
- future native interop boundaries

Disallowed as a general strategy:

- peppering internal compiled FScript execution with repeated dynamic type guards to compensate for weak static typing

## 26. Open Implementation Guidance

Recommended order of implementation:

1. primitive and function types
2. records and arrays
3. blocks and branch typing
4. tagged unions and match exhaustiveness
5. generics
6. generator sequence typing
7. structural equality typing
8. `Unknown` and decoding boundaries
9. effect/type integration

## 27. Summary

The FScript type system is intended to feel familiar to TypeScript users at the surface while behaving much more like a strict native language underneath.

The key properties are:

- structural typing
- immutability
- inference where practical
- explicitness at module boundaries
- no implicit `any`
- no internal runtime type errors in well-typed programs
