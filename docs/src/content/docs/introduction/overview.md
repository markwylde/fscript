---
title: Overview
description: What FScript is, what it keeps from JavaScript and TypeScript, and what kind of language it is trying to be.
---

FScript is a small functional language with syntax that will feel familiar to JavaScript and TypeScript users, but with a much narrower semantic core. It keeps modules, lexical scope, closures, arrow functions, plain records, arrays, structural typing, and tagged unions. It removes classes, prototypes, `this`, `new`, CommonJS, mutation-heavy patterns, and `async` / `await`.

The design goal is not "JavaScript, but stricter." The goal is a language that is easier to reason about, easier to typecheck soundly, and easier to compile to a native runtime.

## Core ideas

- every file is a module
- bindings are immutable
- blocks are expressions
- multi-parameter functions are curried automatically
- effects start eagerly by default
- laziness is explicit through `defer`
- standard-library APIs come from imported `std:` modules, not prototype methods

## A quick example

```fscript
import Array from 'std:array'
import FileSystem from 'std:filesystem'
import Json from 'std:json'
import Result from 'std:result'

type User = {
  id: String,
  name: String,
  active: Boolean,
}

readUsers = (path: String): Result<User[], String> => {
  text = FileSystem.readFile(path)
  value = Json.parse(text)

  match (value) {
    { tag: 'ok', value } => Result.ok(
      value
        |> Array.filter((user) => user.active)
        |> Array.map((user) => ({ id: user.id, name: user.name, active: true }))
    )
    { tag: 'error', error } => Result.error(error.message)
  }
}
```

That example shows several of the language's defaults:

- plain data instead of classes
- left-to-right composition with pipes
- explicit imports for helpers
- typed recoverable failures with `Result`

## If you know TypeScript

FScript will feel familiar in a few important ways:

- type annotations use `:`
- records and arrays look similar
- union and intersection types exist
- arrow functions are the only function form

The biggest mindset shifts are:

- no methods on arrays or objects
- no mutable `let`
- no `return`; the final expression wins
- no `Promise` syntax in source code
- no class or instance model

## What the current project is

The repository currently ships:

- `fscript check` for parsing, name resolution, typechecking, effect analysis, and module-graph validation
- `fscript run` for the broadest current execution path
- `fscript compile` for native executable output, with narrower real-native coverage than `run`

The docs describe the language intentionally, but they also call out where the current implementation is still catching up to the Draft 0.1 specs.

## Where to go next

- Start with [Installation](../getting-started/installation.md) if you want to run the CLI locally.
- Read [Differences from JavaScript](./differences-from-javascript.md) if you are translating an existing JS or TS mental model.
- Jump into the [Language Guide](../language-guide/syntax-overview.md) if you want the day-to-day coding model.
