---
title: Differences from JavaScript
description: The most important ways FScript differs from JavaScript and TypeScript in everyday code.
---

FScript looks familiar on purpose, but it is not a JavaScript compatibility layer. If you come in expecting "TypeScript with a few features removed," the biggest surprises are semantic, not cosmetic.

## No classes, prototypes, or `this`

FScript models data with plain records and behavior with functions.

JavaScript or TypeScript:

```ts
class User {
  constructor(public name: string) {}

  greet() {
    return `hello ${this.name}`
  }
}
```

FScript:

```fscript
type User = {
  name: String,
}

greet = (user: User): String => 'hello ' + user.name
```

That removes instance identity, method dispatch, and prototype lookups from the core model.

## No mutable `let` or `const`

Bindings use plain `=` and are immutable.

JavaScript or TypeScript:

```ts
let count = 0
count = count + 1
```

FScript:

```fscript
count = 0
nextCount = count + 1
```

Instead of updating values in place, you build new values.

## No prototype methods

JavaScript and TypeScript encourage method chains:

```ts
const names = users
  .filter((user) => user.active)
  .map((user) => user.name)
```

FScript uses imported helpers:

```fscript
import Array from 'std:array'

names = users
  |> Array.filter((user) => user.active)
  |> Array.map((user) => user.name)
```

This keeps operations explicit and works naturally with currying and pipes.

## No `function` declarations or `return`

FScript uses arrow functions only, and blocks evaluate to their final expression.

```fscript
greet = (name: String): String => {
  message = 'hello ' + name
  message
}
```

There is no `return` keyword in Draft 0.1.

## No `async` / `await`

FScript treats effects as part of the language runtime model. Source code stays direct:

```fscript
readConfig = (path: String): Config => {
  text = FileSystem.readFile(path)
  Json.parse(text)
}
```

You do not write `async`, `await`, or manipulate `Promise` objects directly. Effects start eagerly by default, and `defer` is the explicit way to delay work.

## No CommonJS

This is valid FScript:

```fscript
import FileSystem from 'std:filesystem'
import { parseUser } from './user.fs'
```

This is not:

```js
const fs = require('fs')
module.exports = {}
```

Each `.fs` file is a module, and module resolution is defined by the FScript compiler and runtime rather than Node.js rules.

## No mutation of arrays or records

These are invalid:

```fscript
user.name = 'Grace'
items[0] = 10
```

Instead, build new values with helpers:

```fscript
import Object from 'std:object'

nextUser = Object.spread(user, { name: 'Grace' })
```

## Different error style

FScript supports `throw` and `try/catch`, but expected failures are usually modeled with `Result<T, E>`:

```fscript
type Result<T, E> =
  | { tag: 'ok', value: T }
  | { tag: 'error', error: E }
```

That makes error handling feel closer to typed data flow than exception-heavy JavaScript code.

## Practical migration mindset

When porting JavaScript or TypeScript code, the most useful habit is to translate by concepts:

- classes become records plus functions
- method chains become pipes plus `std:` helpers
- reassignment becomes new bindings
- promise workflows become direct effectful code
- enum-like state becomes literal types or tagged unions

If you make those shifts early, the rest of the language tends to fit together naturally.
