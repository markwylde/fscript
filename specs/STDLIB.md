# FScript Standard Library Specification

Status: Draft 0.1

## 1. Goal

Define the shape of the built-in standard library for FScript.

The standard library is part of the language distribution and runtime. It is not a userland package manager concern and it does not depend on JavaScript globals or Node.js built-ins.

## 2. Design Principles

The FScript standard library should be:

- explicit
- small
- composable
- data-last where appropriate
- curried by default
- immutable by default
- runtime-backed where host capabilities are needed

Core rule:

- data structures are values
- operations on data structures are imported functions
- prototype methods do not exist

That means this is invalid:

```fs
[1, 2, 3].map((i) => i + 1)
```

And this is correct:

```fs
import Array from 'std:array'

Array.map((i) => i + 1, [1, 2, 3])
```

## 3. Module Model

Built-in modules use the reserved `std:` import scheme.

Examples:

```fs
import Array from 'std:array'
import Object from 'std:object'
import String from 'std:string'
import Number from 'std:number'
import Result from 'std:result'
import Json from 'std:json'
import FileSystem from 'std:filesystem'
import Task from 'std:task'
```

Rules:

- `std:` module names are reserved by the language
- standard-library modules are imported explicitly
- default import is the primary standard pattern for `std:` modules
- user code should not assume the existence of a single catch-all `std` namespace

## 4. Function Shape Conventions

Standard-library functions should follow these conventions unless there is a strong reason not to.

### 4.1 Currying

All standard-library functions are curried by default because the language is curried by default.

Example:

```fs
addNumbers = Array.map((i) => i + 1)
result = addNumbers([1, 2, 3])
```

### 4.2 Data-last design

Transformation functions should place their primary data input last.

Example:

```fs
Array.map((i) => i + 1, [1, 2, 3])
```

This supports both partial application and pipe syntax:

```fs
[1, 2, 3]
  |> Array.map((i) => i + 1)
  |> Array.filter((i) => i > 2)
```

### 4.3 Immutability

Standard-library functions must never mutate caller-visible data.

All collection updates return new values.

## 5. Required Draft 0.1 Modules

The minimum built-in module set for Draft 0.1 is:

- `std:array`
- `std:object`
- `std:string`
- `std:number`
- `std:result`
- `std:json`
- `std:filesystem`
- `std:task`

Additional modules may be added later, but these form the initial standard surface.

## 6. `std:array`

Purpose:

- immutable array construction and transformation
- pipeline-friendly collection helpers

Representative API:

```fs
Array.map = <T, U>(fn: (value: T): U, items: T[]): U[]
Array.filter = <T>(fn: (value: T): Boolean, items: T[]): T[]
Array.reduce = <T, U>(fn: (state: U, value: T): U, initial: U, items: T[]): U
Array.forEach = <T>(fn: (value: T): Undefined, items: T[]): Undefined
Array.length = <T>(items: T[]): Number
Array.append = <T>(value: T, items: T[]): T[]
Array.concat = <T>(right: T[], left: T[]): T[]
Array.at = <T>(index: Number, items: T[]): T | Null
Array.slice = <T>(start: Number, end: Number, items: T[]): T[]
Array.flatMap = <T, U>(fn: (value: T): U[], items: T[]): U[]
```

Examples:

```fs
import Array from 'std:array'

names = users
  |> Array.filter((user) => user.active)
  |> Array.map((user) => user.name)
```

## 7. `std:object`

Purpose:

- immutable record helpers
- explicit object composition without prototype methods

Representative API:

```fs
Object.spread = <T>(parts: T[]): T
Object.keys = <T>(value: T): String[]
Object.values = <T>(value: T): Unknown[]
Object.entries = <T>(value: T): { key: String, value: Unknown }[]
Object.has = <T>(key: String, value: T): Boolean
Object.get = <T>(key: String, value: T): Unknown | Null
Object.set = <T, V>(key: String, fieldValue: V, value: T): T
```

Notes:

- `Object.spread` should merge left-to-right, with later fields replacing earlier fields
- record update helpers must preserve immutability semantics
- type precision for dynamic-key helpers may improve over time; Draft 0.1 may be conservative at the type level

Example:

```fs
import Object from 'std:object'

base = { a: 1 }
next = Object.spread(base, { b: 2 })
```

## 8. `std:string`

Purpose:

- string transformation and inspection

Representative API:

```fs
String.length = (value: String): Number
String.uppercase = (value: String): String
String.lowercase = (value: String): String
String.trim = (value: String): String
String.split = (separator: String, value: String): String[]
String.join = (separator: String, values: String[]): String
String.startsWith = (prefix: String, value: String): Boolean
String.endsWith = (suffix: String, value: String): Boolean
String.contains = (part: String, value: String): Boolean
String.isDigits = (value: String): Boolean
```

## 9. `std:number`

Purpose:

- numeric helpers and parsing

Representative API:

```fs
Number.parse = (value: String): Number
Number.toString = (value: Number): String
Number.floor = (value: Number): Number
Number.ceil = (value: Number): Number
Number.round = (value: Number): Number
Number.min = (right: Number, left: Number): Number
Number.max = (right: Number, left: Number): Number
Number.clamp = (min: Number, max: Number, value: Number): Number
```

Notes:

- functions that can fail in ordinary ways should prefer `Result`-returning variants in future additions
- Draft 0.1 may keep some basic parsing helpers simple while higher-level APIs use `Result`

## 10. `std:result`

Purpose:

- typed expected-failure handling
- the preferred model for recoverable errors

Core type shape:

```fs
type Result<T, E> =
  | { tag: 'ok', value: T }
  | { tag: 'error', error: E }
```

Representative API:

```fs
Result.ok = <T, E>(value: T): Result<T, E>
Result.error = <T, E>(error: E): Result<T, E>
Result.map = <T, U, E>(fn: (value: T): U, result: Result<T, E>): Result<U, E>
Result.mapError = <T, E, F>(fn: (error: E): F, result: Result<T, E>): Result<T, F>
Result.andThen = <T, U, E>(fn: (value: T): Result<U, E>, result: Result<T, E>): Result<U, E>
Result.withDefault = <T, E>(fallback: T, result: Result<T, E>): T
Result.isOk = <T, E>(result: Result<T, E>): Boolean
Result.isError = <T, E>(result: Result<T, E>): Boolean
```

Example:

```fs
import Result from 'std:result'

parsePort = (text: String): Result<Number, { tag: 'parse_error', message: String }> => {
  if (String.isDigits(text)) {
    Result.ok(Number.parse(text))
  } else {
    Result.error({ tag: 'parse_error', message: 'invalid port' })
  }
}
```

## 11. `std:json`

Purpose:

- JSON parsing and serialization
- explicit boundary between untyped external data and typed program values

Representative API:

```fs
Json.parse = (text: String): Unknown
Json.stringify = (value: Unknown): String
Json.decode = <T>(decoder: Decoder<T>, value: Unknown): Result<T, DecodeError>
Json.parseAs = <T>(decoder: Decoder<T>, text: String): Result<T, DecodeError>
```

Notes:

- `Json.parse` should not pretend arbitrary JSON is already typed program data
- decoding should be explicit
- `Decoder<T>` may be a standard-library abstraction introduced in Draft 0.1 or Draft 0.2

## 12. `std:filesystem`

Purpose:

- native file IO through the runtime
- effectful host capability access

Representative API:

```fs
FileSystem.readFile = (path: String): String
FileSystem.writeFile = (path: String, content: String): Undefined
FileSystem.exists = (path: String): Boolean
FileSystem.deleteFile = (path: String): Undefined
FileSystem.readDir = (path: String): String[]
```

Semantics:

- these functions are effectful
- effectful calls start eagerly by default
- they participate in implicit suspension/resolution semantics
- they are implemented by the native FScript runtime, not Node.js

Example:

```fs
import FileSystem from 'std:filesystem'

loadText = (path: String): String => FileSystem.readFile(path)
```

## 13. `std:task`

Purpose:

- explicit concurrency and task control when implicit dependency scheduling is not enough

Representative API:

```fs
Task.all = <T>(tasks: Task<T>[]): T[]
Task.race = <T>(tasks: Task<T>[]): T
Task.spawn = <T>(task: Task<T>): Task<T>
Task.force = <T>(deferred: Deferred<T>): T
Task.defer = <T>(fn: (): T): Deferred<T>
```

Notes:

- the language has native `defer`, so `Task.defer` may be redundant in user-facing code; it may still be useful as a library/runtime-level primitive
- `Task` and `Deferred` are runtime concepts surfaced carefully to user code
- Draft 0.1 should keep this module minimal until the implicit async model is stable

## 14. Curried Usage Examples

```fs
import Array from 'std:array'
import Object from 'std:object'

addNumbers = Array.map((i) => i + 1)
mergeUser = Object.spread({ active: true })

numbers = addNumbers([1, 2, 3])
user = mergeUser({ name: 'Ada' })
```

## 15. Pipe Usage Examples

```fs
import Array from 'std:array'
import String from 'std:string'

result = ['  ada  ', '  grace  ']
  |> Array.map(String.trim)
  |> Array.map(String.uppercase)
```

## 16. Error Handling Guidance

The standard library should prefer:

- `Result<T, E>` for expected failures
- thrown values only for exceptional boundaries or unrecoverable runtime situations

Examples:

- parsing user input should prefer `Result`
- missing files may eventually expose `Result`-returning variants depending on the final ergonomics decision
- internal runtime corruption is not a `Result`; it is an exceptional runtime failure

## 17. Module Growth Policy

New standard-library modules should be added carefully.

Questions to ask before adding a module:

- is this general-purpose language functionality?
- does it belong in the core runtime distribution?
- does it preserve the explicit, immutable, data-last design?
- can it be explained simply to users coming from JS/TS?

## 18. Implementation Guidance

The Rust implementation should:

- implement `std:` modules as native runtime-backed modules
- test each std module independently
- ensure immutable semantics are preserved
- ensure currying and pipe usage work consistently across APIs
- keep host boundaries explicit and measurable

## 19. Summary

The FScript standard library is part of the language model, not an afterthought.

Its core identity is:

- explicit module imports
- no prototype methods
- curried data-last APIs
- immutable data operations
- `Result` for expected failure
- runtime-backed host capabilities for effects
