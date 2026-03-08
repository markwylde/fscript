---
title: Overview
description: What FScript is, what it keeps from JavaScript and TypeScript, and what kind of language it is trying to be.
---

FScript is a reduced, functional descendant of ECMAScript and TypeScript. It keeps familiar syntax where that helps readability, but it intentionally drops large parts of the JavaScript runtime model.

## FScript keeps

- modules
- lexical scope
- closures
- arrow functions
- destructuring
- pattern matching
- structural types
- union and intersection types
- object and array literals

## FScript removes

- classes
- interfaces
- enums
- prototypes
- `this`
- `new`
- `instanceof`
- CommonJS

## Big idea

FScript wants sequential-looking source code with a smaller and more predictable semantic core. Pure code evaluates immediately. Effectful code is handled by the runtime. Laziness is explicit through `defer`.

