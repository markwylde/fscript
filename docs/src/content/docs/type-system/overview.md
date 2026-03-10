---
title: Type System Overview
description: A strict, structural, mostly inferred type system designed for predictable code and stronger native compilation guarantees.
---

FScript's type system is strict, structural, predictable, and mostly inferred. It aims to feel familiar to TypeScript users while giving the compiler stronger guarantees.

## Main goals

- reject ill-typed programs before execution
- avoid implicit `any`
- support structural typing for plain data
- keep local code lightweight through inference where that is safe
- make exported APIs explicit and readable

## The core type categories

- primitive types
- record types
- array types
- function types
- generator sequence types
- union and intersection types
- literal types
- generic types
- `Unknown`
- `Never`

## Comparison to TypeScript

The overall feel is intentionally familiar, but Draft 0.1 is stricter in several ways:

- no implicit `any`
- more emphasis on explicit annotations at module boundaries
- stronger preference for closed record shapes
- a design goal of avoiding internal runtime type mismatches in well-typed code

## Best reading order

1. [Inference](./inference.md)
2. [Primitive Types](./primitive-types.md)
3. [Records](./records.md)
4. [Functions](./functions.md)
5. [Unions](./unions.md)
6. [Tagged Unions](./tagged-unions.md)
