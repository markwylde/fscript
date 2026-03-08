---
title: Type System Overview
description: A strict, structural, mostly inferred type system designed for predictable code and stronger native compilation guarantees.
---

FScript's type system is strict, structural, predictable, and mostly inferred. It aims to feel familiar to TypeScript users while giving the compiler stronger guarantees.

## Goals

- reject ill-typed programs at compile time
- avoid implicit `any`
- keep internal well-typed code free from runtime type mismatches
- support immutable data and functional composition

