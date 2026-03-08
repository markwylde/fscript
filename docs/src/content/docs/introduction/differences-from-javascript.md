---
title: Differences from JavaScript
description: The most important ways FScript differs from JavaScript and TypeScript in everyday code.
---

FScript looks familiar on purpose, but it is not a compatibility layer for JavaScript.

## No classes or prototypes

There are no classes, no prototype methods, no `this`, and no `new`.

## No `let`, `const`, or `var`

Bindings use plain `=` and are immutable.

```fs
answer = 42
```

## No `return`

Block-bodied functions evaluate to their final expression.

```fs
greet = (name: String): String => {
  'hello ' + name
}
```

## No `async` and `await`

Effects are handled by the runtime model instead of promise syntax in source code.

## No prototype methods

This is invalid:

```fs
[1, 2, 3].map((i) => i + 1)
```

This is the FScript style:

```fs
import Array from 'std:array'

Array.map((i) => i + 1, [1, 2, 3])
```

## No CommonJS

Use `import` and `export`. `require` and `module.exports` are not part of the language.

