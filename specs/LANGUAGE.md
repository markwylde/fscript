# FScript Language Specification

Status: Draft 0.1

## 1. Overview

FScript is a reduced, functional descendant of ECMAScript and TypeScript.

It keeps the parts of the modern JavaScript and TypeScript model that are useful for a small, expressive, typed language:

- modules
- lexical scope
- closures
- arrow functions
- currying
- generators
- destructuring
- pattern matching
- structural types
- union and intersection types
- object and array literals
- expression-oriented composition

It removes the object-oriented and prototype-driven parts of the language:

- no classes
- no interfaces
- no enums
- no prototypes
- no `this`
- no `new`
- no `instanceof`
- no decorators
- no method borrowing via `bind`, `call`, or `apply`

FScript is async-first, single-threaded, and functional by default.

The core idea is:

- pure expressions evaluate immediately
- effectful operations suspend implicitly
- the programmer does not write `async` or `await`
- effectful calls start eagerly by default
- laziness is explicit through `defer`
- chaining is built around a pipe operator and explicit standard-library functions

## 2. Design Goals

FScript is designed to be:

- small: a reduced language core instead of full ECMAScript compatibility
- functional: functions, immutable values, and composition come first
- curried: multi-argument functions partially apply by default
- explicit: collection and object helpers come from imported modules, not hidden prototypes
- async-first: IO and other effects are asynchronous by semantics, without `Promise` syntax
- typed: structural typing remains a first-class part of the language
- portable: suitable for `fscript run file.fs` and `fscript compile file.fs output`

## 3. Non-Goals

FScript does not attempt to be source-compatible with JavaScript or TypeScript.

Specifically, FScript does not preserve:

- CommonJS (`require`, `module.exports`)
- prototype-based inheritance
- class-based APIs
- method-style collection helpers such as `[].map(...)`
- the JavaScript `Promise` programming model as a source-language primitive
- mutation-heavy programming patterns

## 4. Core Model

A FScript program is a graph of expressions, values, and effects.

The language has three semantic layers:

1. Pure values and expressions.
2. Effectful operations such as file IO, network IO, time, randomness, and process interaction.
3. A single-threaded scheduler that executes effectful work when dependencies are ready.

The programmer writes sequential-looking code. The compiler and runtime decide when effectful work can begin, while preserving observable ordering rules.

## 4.1 Soundness and Performance Goals

FScript is designed to trade JavaScript's dynamic flexibility for stronger compile-time guarantees and more predictable performance.

Draft 0.1 aims for the following properties:

- well-typed programs are accepted at compile time
- ill-typed programs are rejected before execution begins
- internal type errors in well-typed FScript code should not occur at runtime
- runtime checks should be concentrated at host boundaries such as file input, parsing, decoding, and future native interop boundaries
- pure code should compile to direct code without async state-machine overhead
- only effectful code should be lowered into suspendable runtime machinery

These goals exist because FScript removes many of the runtime shape-changing behaviors that make JavaScript engines rely heavily on speculation and deoptimization:

- no prototypes
- no class instances
- no hidden method dispatch through object inheritance
- immutable bindings
- immutable records and arrays
- explicit standard-library calls instead of prototype methods

This creates room for a compiler and runtime to make stronger assumptions about:

- record layout
- array layout
- function specialization
- field access
- elimination of repeated type checks

Draft 0.1 does not guarantee that every FScript program is faster than every JavaScript program.

However, the language is intentionally designed so that an ahead-of-time native implementation can outperform JavaScript in many workloads, especially when startup cost, predictable optimization, fixed data shapes, and removal of dynamic runtime guards matter.

## 5. Source Files and Modules

A source file uses the `.fs` extension.

Each file is a module.

Supported module forms:

```fs
import Array from 'std:array'
import Object from 'std:object'
import FileSystem from 'std:filesystem'
import { parseUser } from './user.fs'

export readUser = (path: String): User => {
  const text = FileSystem.readFile(path)
  parseUser(text)
}
```

Rules:

- `import` is supported.
- `export` is supported.
- `require` is not supported.
- named exports are preferred for user modules.
- default exports are allowed.
- default exports are especially appropriate for `std:` modules.
- CommonJS module semantics are not supported.
- Module resolution is defined by the compiler and runtime, not by Node.js.
- top-level module code executes once when the module is loaded.
- circular imports are a compile error in Draft 0.1.

## 6. Bindings and Immutability

FScript uses immutable bindings by default.

```fs
answer = 42
name = 'Ada'
```

Rules:

- a binding is created with `name = expression`
- bindings are immutable
- bindings are block-scoped
- rebinding the same name in the same scope is a compile error
- rebinding in a nested inner scope is allowed
- `var` is not supported.
- `let` is not supported.
- `const` is not supported.

Records and arrays are also immutable in Draft 0.1.

That means field assignment and index assignment are invalid:

```fs
user.name = 'Grace' // invalid
items[0] = 10 // invalid
```

Updates are expressed by creating new values:

```fs
base = { a: 1 }
next = Object.spread(base, { b: 2 })
```

## 7. Functions

Functions are written with arrow syntax only.

```fs
add = (a: Number, b: Number): Number => a + b

greet = (name: String): String => {
  'hello ' + name
}
```

Rules:

- The `function` keyword is not supported.
- Arrow functions are the only function syntax.
- Functions are first-class values.
- Closures are supported.
- Functions are curried by default.
- Generator arrows are supported.
- Functions may be pure or effectful.
- The compiler infers whether a function is pure or effectful.

Examples:

```fs
makeAdder = (x: Number) => (y: Number): Number => x + y

add5 = makeAdder(5)
result = add5(3)
```

### 7.1 Default Currying

Every multi-parameter function is curried automatically.

These forms are semantically equivalent:

```fs
add = (a: Number, b: Number): Number => a + b
```

```fs
add = (a: Number) => (b: Number): Number => a + b
```

This means partial application is always available:

```fs
add10 = add(10)
value = add10(5)
```

Calling a function with fewer arguments than its full arity returns another function that accepts the remaining arguments.

Draft 0.1 treats extra arguments as a type error.

### 7.2 Implicit Return

Block-bodied arrow functions evaluate to their final expression.

Example:

```fs
test = () => {
  a = 1
  a
}
```

There is no `return` keyword in Draft 0.1.

### 7.3 Generators and `yield`

FScript supports generators.

Because the language does not support the `function` keyword, generators use arrow-based syntax.

Draft 0.1 defines generator arrows with a leading `*`:

```fs
numbers = *() => {
  yield 1
  yield 2
  yield 3
}
```

Rules:

- `yield` is only valid inside a generator arrow
- a generator arrow produces a lazy `Sequence<T>` value
- generator execution is suspended after each `yield`
- a generator completes when its body reaches the end
- generator bodies do not use `return`
- generator arrows are intended for pure lazy iteration
- yielding effectful computations from a generator arrow is invalid in Draft 0.1
- asynchronous streaming is a separate concept from generators and is expected to use a `Stream<T>` abstraction in the standard library

Example:

```fs
pair = *(a: Number, b: Number) => {
  yield a
  yield b
}
```

## 8. Values and Data Structures

FScript supports the following value forms in the core language:

- `Number`
- `String`
- `Boolean`
- `Null`
- `Undefined`
- arrays
- records
- tagged unions through type aliases
- functions

Examples:

```fs
point = { x: 1, y: 2 }
names = ['Ada', 'Grace', 'Linus']
```

Records are plain data containers.

Arrays are plain indexed collections.

Records and arrays are immutable values.

Neither records nor arrays inherit from prototypes.

That means instance methods do not exist:

```fs
[].map // invalid
({}).hasOwnProperty // invalid
```

Property access is allowed only for explicit data fields:

```fs
user = { name: 'Ada', age: 30 }
name = user.name
```

## 9. Standard Library over Prototypes

Behavior that would normally appear as instance methods in JavaScript must instead come from imported standard-library namespaces.

Example:

```fs
import Array from 'std:array'
import Object from 'std:object'

result = Array.map((i) => i + 1, [1, 2, 3])
merged = Object.spread({ a: 1 }, { b: 2 }, { c: 3 })
```

Design rule:

- data structures are values
- operations on data structures are functions in modules
- standard-library collection helpers should place data in the final argument position

This keeps behavior explicit and avoids hidden inheritance.

This convention exists so helpers work well with both currying and the pipe operator.

Example:

```fs
import Array from 'std:array'

addNumbers = Array.map((i) => i + 1)
result = addNumbers([1, 2, 3])
```

### 9.1 Standard Library Modules

The standard library is part of the language distribution.

Draft 0.1 assumes built-in modules addressed through the `std:` scheme.

Examples:

```fs
import Array from 'std:array'
import Object from 'std:object'
import String from 'std:string'
import Number from 'std:number'
import Option from 'std:option'
import Result from 'std:result'
import FileSystem from 'std:filesystem'
import Http from 'std:http'
import Task from 'std:task'
```

Rules:

- standard-library modules are imported explicitly
- there is no single catch-all `std` namespace
- module names under `std:` are reserved by the language
- the default export of a `std:` module is the namespace object used by source code

The exact contents of these namespaces are defined in separate standard-library specifications.

## 10. Chaining and the Pipe Operator

Chaining is a core feature.

FScript uses a pipe operator:

```fs
value |> fn(...)
```

Semantics:

```fs
value |> fn(a, b)
```

desugars to:

```fs
fn(a, b, value)
```

The piped value is inserted as the final argument.

This makes data-last standard-library APIs natural to chain.

Example:

```fs
import Array from 'std:array'

result = [1, 2, 3]
  |> Array.map((i) => i + 1)
  |> Array.filter((i) => i > 2)
```

Equivalent desugaring:

```fs
result = Array.filter(
  (i) => i > 2,
  Array.map((i) => i + 1, [1, 2, 3])
)
```

Currying and piping are designed to work together.

These are equivalent:

```fs
addNumbers = Array.map((i) => i + 1)
result = addNumbers([1, 2, 3])
```

```fs
result = [1, 2, 3] |> Array.map((i) => i + 1)
```

Rules:

- pipe is left-associative
- pipe does not call methods
- pipe is syntax sugar for function application

## 11. Types

FScript keeps a reduced structural type system inspired by TypeScript.

Supported type forms in Draft 0.1:

- primitive types
- record types
- array types
- function types
- union types
- intersection types
- generic type parameters
- literal types
- type aliases

Examples:

```fs
type Point = { x: Number, y: Number }

type User = {
  id: String,
  name: String,
}

type Maybe<T> = T | Null

type Mapper<T, U> = (value: T): U
```

Rules:

- typing is structural
- `interface` is not supported
- `enum` is not supported
- `class` types are not supported
- nominal typing is not part of Draft 0.1

### 11.1 Recommended Built-in Type Names

Draft 0.1 uses the following canonical names in examples:

- `Number`
- `String`
- `Boolean`
- `Null`
- `Undefined`
- `Never`
- `Unknown`

Whether the surface syntax also supports lowercase aliases such as `number` and `string` is an implementation detail and should be standardized separately.

## 12. Tagged Unions

Because there are no classes or enums, sum types are modeled with tagged unions.

Example:

```fs
type User =
  | { tag: 'guest' }
  | { tag: 'member', id: String, name: String }
```

Tagged unions are the preferred way to model variants.

## 13. Expressions

Draft 0.1 supports these expression categories:

- literals
- identifier references
- function calls
- arrow functions
- `yield` expressions inside generator arrows
- record literals
- array literals
- property access
- index access
- unary operators
- binary operators
- conditional expressions
- pipe expressions
- `match` expressions
- `if` expressions
- `try` / `catch` expressions

Representative operators include:

- `+`
- `-`
- `*`
- `/`
- `%`
- `&&`
- `||`
- `??`
- `===`
- `!==`
- `<`
- `<=`
- `>`
- `>=`

Draft 0.1 does not include:

- `instanceof`
- `in`
- `delete`
- `new`
- optional chaining on prototype methods

### 13.1 Match Expressions

Because Draft 0.1 does not include `switch`, branching over tagged unions is done with `match`.

Example:

```fs
label = match (user) {
  { tag: 'guest' } => 'Guest',
  { tag: 'member', name } => name,
}
```

Rules:

- `match` is an expression
- `match` should be exhaustive for tagged unions
- patterns may destructure records and arrays

### 13.2 Destructuring

FScript supports destructuring in bindings, parameter lists, and match patterns.

Examples:

```fs
{ name, age } = user
[head, tail] = items
greet = ({ name }: User): String => 'hello ' + name
```

### 13.3 Equality

`===` and `!==` compare plain data structurally in Draft 0.1.

That means:

- numbers, strings, booleans, `Null`, and `Undefined` compare by value
- records compare structurally
- arrays compare structurally
- tagged unions compare structurally
- functions are not comparable
- generator and stream values are not comparable

This is intentionally different from JavaScript reference identity.

## 14. Statements and Control Flow

FScript is expression-first.

Blocks are evaluated top-to-bottom, and the final expression becomes the value of the block.

Within a block, Draft 0.1 supports:

- immutable bindings
- `if` expressions
- `match` expressions
- `throw`
- `try` / `catch` expressions

Example:

```fs
describe = (value: Number): String => {
  if (value > 10) {
    'big'
  } else {
    'small'
  }
}
```

Rules:

- `if` is an expression
- `else` is required when the value of an `if` is used
- `try` / `catch` is an expression
- the selected branch of an `if` or `try` / `catch` evaluates to the value of its final expression

Iteration is intended to happen primarily through standard-library functions rather than loop statements.

Draft 0.1 therefore does not require support for:

- `for`
- `while`
- `do`

These may be added later only if they fit the functional execution model cleanly.

## 15. Object Spread and Collection Helpers

To keep collection behavior explicit, Draft 0.1 prefers standard-library functions over special syntax where practical.

That means:

- array transformation is done via `Array.*`
- object merging and spreading is done via `Object.*`

Example:

```fs
import Object from 'std:object'

config = Object.spread(defaults, envConfig, localConfig)
```

Draft 0.1 leaves open whether `{ ...a, ...b }` syntax should exist.

The recommended direction is:

- keep the core grammar small
- prefer `Object.spread(...)` in user code

## 16. Purity and Effects

FScript is pure by default.

A function is pure if it:

- depends only on its arguments
- does not perform IO
- does not access time or randomness
- does not mutate observable shared state

Example:

```fs
add = (a: Number, b: Number): Number => a + b
```

A function is effectful if it performs operations such as:

- file IO
- network IO
- process IO
- reading the current time
- generating randomness
- logging

Example:

```fs
import FileSystem from 'std:filesystem'

loadText = (path: String): String => FileSystem.readFile(path)
```

In Draft 0.1:

- effect information is tracked by the compiler
- user code does not write `async`
- user code does not write `await`
- user code does not construct or consume `Promise`

### 16.1 Effect Visibility

Draft 0.1 infers effects rather than requiring source annotations.

This keeps authoring close to JavaScript and TypeScript while still allowing the compiler to distinguish pure code from effectful code.

Rules:

- local functions do not need effect annotations
- if a function calls an effectful function, it becomes effectful
- tooling, generated documentation, and compiler diagnostics should surface inferred effects for exported functions

A future version may expose optional effect annotations such as:

```fs
loadText = (path: String): String !FileSystem => FileSystem.readFile(path)
```

## 17. Async by Semantics

Effectful operations are asynchronous by semantics.

Example:

```fs
import FileSystem from 'std:filesystem'

something = (): String => {
  filepath = '/tmp/test.txt'
  content = getContent()
  FileSystem.writeFile(filepath, content)
  content
}
```

The programmer writes this as normal sequential code.

The runtime interprets it approximately as:

1. evaluate `filepath` immediately
2. start `getContent()` immediately
3. suspend until `content` is available where needed
4. start `FileSystem.writeFile(filepath, content)` once its dependencies are ready
5. resolve the function with `content`

### 17.1 Key Rule

Every effectful call yields a value that is implicitly awaited before it is consumed.

This means:

- no explicit `await` syntax is required
- values from effectful calls behave like ordinary values in source code
- the scheduler inserts suspension points automatically
- effectful calls start eagerly when execution reaches them

### 17.2 `defer`

Draft 0.1 includes a native `defer` form for explicit laziness.

Example:

```fs
a = defer getA()
b = defer getB()
```

Rules:

- ordinary effectful calls start eagerly
- `defer expr` delays starting `expr` until the deferred value is invoked or forced
- `defer` is the preferred way to opt out of eager start
- `defer` is not a replacement for generators or streams

### 17.2 Pure vs Effectful Evaluation

The runtime must distinguish between:

- immediate expressions, such as `1 + 2`
- suspendable operations, such as `FileSystem.readFile(path)`

Pure arithmetic, record construction, and local transformations are not turned into async tasks.

Only effectful operations participate in implicit suspension.

## 18. Dependency-Driven Scheduling

The compiler or runtime may build a dependency graph from local bindings.

Example:

```fs
a = getA()
b = getB()
c = combine(a, b)
c
```

The scheduler may start `getA()` and `getB()` as soon as their arguments are available.

`combine(a, b)` may begin only after both values resolve.

This allows automatic overlap of independent effectful work without requiring `Promise.all(...)` in source code.

## 19. Observable Ordering Rules

Automatic scheduling must not violate observable behavior.

Therefore Draft 0.1 defines the following rule:

- Pure computation may be freely reordered when dependencies allow.
- Effectful computation must preserve source order unless the compiler can prove reordering is unobservable or the programmer opts into explicit parallel composition.

Example:

```fs
log('start')
doWork()
log('end')
```

These operations must preserve source order.

Example:

```fs
const user = fetchUser()
const settings = fetchSettings()
const result = merge(user, settings)
```

These fetches may overlap if the compiler classifies them as independently schedulable.

## 20. Explicit Concurrency

Draft 0.1 removes `Promise` from the source language, but it does not remove the need for explicit concurrency tools.

The language therefore reserves room for standard-library or future-syntax concurrency primitives such as:

- `Task.all(...)`
- `Task.race(...)`
- `Task.spawn(...)`
- `parallel { ... }`

These are not required in the minimal core, but they are expected to exist in the ecosystem.

The important distinction is:

- source code should not be built around `Promise`
- concurrency control may still be exposed explicitly where needed

## 21. Error Handling

FScript has no `Error` class hierarchy.

Because there is no `class` and no `new`, error values are plain data.

Thrown values should typically be tagged records.

Draft 0.1 distinguishes between:

- expected failures, which should use `Result<T, E>`
- exceptional failures, which may use `throw`

Example:

```fs
type ParseError = {
  tag: 'parse_error',
  message: String,
}

parsePort = (text: String): Result<Number, ParseError> => {
  if (String.isDigits(text)) {
    Result.ok(Number.parse(text))
  } else {
    Result.error({ tag: 'parse_error', message: 'invalid port' })
  }
}
```

Rules:

- `throw` is supported.
- `throw new Error(...)` is invalid.
- built-in error classes are not part of the source language model.
- `catch` receives the thrown value directly.
- expected failure should prefer `Result<T, E>`
- `throw` is an escape hatch for exceptional boundaries and unrecoverable situations

## 22. Unsupported ECMAScript and TypeScript Features

The following constructs are intentionally outside Draft 0.1:

- `const`
- `function` declarations
- `class`
- `extends`
- `implements`
- `constructor`
- `interface`
- `enum`
- `namespace`
- decorators
- `prototype`
- `this`
- `super`
- `new`
- `instanceof`
- `var`
- `let`
- `async`
- `await`
- `Promise`
- `return`
- `switch`
- `require`
- `module.exports`
- `bind`
- `call`
- `apply`
- assignment to an existing binding
- property assignment
- index assignment

## 23. Example Program

```fs
import Array from 'std:array'
import Json from 'std:json'
import FileSystem from 'std:filesystem'
import Logger from 'std:logger'
import Result from 'std:result'

type User = {
  id: String,
  name: String,
  active: Boolean,
}

parseUsers = (text: String): User[] => {
  Json.jsonToObject(text)
}

export loadActiveNames = (path: String): String[] => {
  text = FileSystem.readFile(path)
  users = parseUsers(text)
  logger = Logger.create({
    name: 'users',
    level: 'info',
    destination: 'stdout',
  })
  logged = Logger.log(logger, 'loaded users')

  users
    |> Array.filter((user) => user.active)
    |> Array.map((user) => user.name)
}
```

This example shows the intended shape of the language:

- imported namespaces instead of prototype methods
- arrow functions only
- type aliases instead of interfaces or classes
- plain records instead of instances
- immutable bindings and immutable data
- async file IO without `async` / `await`
- chaining through `|>`

## 24. Runtime and Host Model

FScript does not depend on a JavaScript runtime.

Draft 0.1 assumes:

- the compiler is implemented in Rust
- the runtime is implemented in Rust
- the standard library is implemented against native runtime capabilities
- `std:` modules such as `std:filesystem` are provided by the FScript runtime, not by Node.js or browser globals

Native host integration beyond the standard library is a future concern and is not part of the core language specification.

## 25. Implementation Notes for the Compiler

A minimal compiler for Draft 0.1 should be able to:

1. tokenize and parse `.fs` files
2. build an AST for modules, types, and expressions
3. perform name resolution and module resolution
4. infer or validate structural types
5. classify pure and effectful calls
6. lower pipe expressions into ordinary calls
7. lower implicit async semantics into an internal task representation
8. execute in interpreter mode for `fscript run file.fs`
9. emit a standalone executable for `fscript compile file.fs output`

This section is non-normative, but it reflects the intended architecture of the language.

## 26. Summary

FScript Draft 0.1 is:

- a reduced ECMAScript and TypeScript descendant
- functional and immutable by default
- structural and typed
- async-first without `async`, `await`, or `Promise` in user code
- eager by default, with native `defer` for laziness
- explicit about collection behavior through imported standard-library modules
- centered on pipe-based chaining instead of prototype methods
- built on a native Rust runtime rather than a JavaScript host

A short slogan for the design is:

Pure by default, effectful by type, async by semantics.
