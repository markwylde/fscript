---
title: Design Goals
description: The main goals that shape FScript's syntax, type system, runtime model, and standard library.
---

FScript is designed to be small, functional, explicit, typed, and portable. Those words are not branding terms; they drive concrete language decisions.

## Small

FScript intentionally has a reduced core:

- no classes
- no prototype inheritance
- no method-heavy object model
- no CommonJS
- no second syntax for function declarations

The language tries to keep the useful parts of the JavaScript and TypeScript surface while cutting away features that make static reasoning and native compilation harder.

## Functional

The default programming style is value-oriented and compositional:

- bindings do not change
- arrays and records are immutable
- functions are first-class
- multi-parameter functions are curried automatically
- blocks produce values instead of depending on statement sequencing

This gives FScript a very different center of gravity from typical application JavaScript, even though some syntax looks familiar.

## Explicit

FScript avoids hidden runtime behavior.

Examples:

- collection helpers live in `std:array`, not on `Array.prototype`
- record composition uses explicit helpers such as `Object.spread`
- laziness uses `defer`, not accidental promise creation
- host capabilities like filesystem access come from explicit imports

That explicitness helps readability and keeps more behavior visible to the compiler.

## Typed

The type system aims to feel familiar to TypeScript users while being stricter and more predictable:

- no implicit `any`
- structural typing for plain data
- union, intersection, generic, and literal types
- narrowing through control flow and `match`
- stronger expectations around annotations at module boundaries

The long-term goal is that well-typed FScript code should not hit internal type mismatches at runtime.

## Async-by-semantics

FScript does not use `async` / `await` syntax. Instead:

- pure expressions evaluate immediately
- effectful calls start eagerly when reached
- the runtime suspends only when a value is actually needed and not ready
- `defer` is the explicit escape hatch for laziness

This is one of the biggest differences from JavaScript. The language wants sequential-looking source with a runtime that understands effects directly.

## Portable

The intended toolchain is native:

- `fscript run file.fs`
- `fscript compile file.fs output`
- runtime support implemented in Rust
- no JavaScript runtime dependency required by the language design

Portability here means "small native toolchain and predictable runtime contract," not "run inside every existing JS environment unchanged."

## Practical reading guide

When you are learning FScript, these goals explain many of the "why" answers:

- if you wonder why there are no prototype methods, that is `explicit`
- if you wonder why bindings do not reassign, that is `functional`
- if you wonder why `return` is absent, that is `small` plus `expression-oriented`
- if you wonder why `Result` is preferred for recoverable failures, that is `typed`

The rest of the docs expand these goals into day-to-day language rules.
