---
title: Design Goals
description: The main goals that shape FScript's syntax, type system, runtime model, and standard library.
---

FScript is designed to be small, functional, explicit, typed, and portable.

## Small

The language aims for a reduced core instead of broad JavaScript compatibility.

## Functional

Functions, immutable values, and composition come first. Prototype-heavy and object-oriented patterns do not.

## Explicit

Collection and object helpers come from imported modules, not hidden prototype methods.

## Typed

The type system is structural, mostly inferred, and strict enough to support ahead-of-time native compilation.

## Async-first

The runtime handles effects as part of language semantics. The programmer does not write `async` and `await`.

