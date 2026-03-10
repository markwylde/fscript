---
title: Your First Program
description: Write a small .fs file, check it, and run it through the current FScript CLI.
---

Create a file called `hello.fs`:

```fscript
import Logger from 'std:logger'

greet = (name: String): String => 'hello ' + name

main = (): Undefined => {
  Logger.info(greet('FScript'))
}

main()
```

That program already shows a few core rules:

- imports are explicit
- functions use arrow syntax
- the final expression in a block becomes the result
- logging comes from `std:logger`, not a global `console`

## Check it first

```bash
cargo run -p fscript-cli -- check hello.fs
```

If the file parses and typechecks, the CLI exits successfully.

## Run it

```bash
cargo run -p fscript-cli -- run hello.fs
```

You should see the greeting printed through the current runtime-backed logger.

## A slightly richer version

```fscript
import Array from 'std:array'
import Logger from 'std:logger'

names = ['Ada', 'Grace', 'Linus']

messages = names
  |> Array.map((name) => 'hello ' + name)

main = (): Undefined => {
  Array.forEach((message) => Logger.info(message), messages)
}

main()
```

That example introduces pipes and explicit array helpers.

## If you know JavaScript

A similar JavaScript example might use:

- `console.log`
- `names.map(...)`
- `function main() { return ... }`

In FScript the same ideas become:

- `Logger.info`
- `Array.map`
- arrow functions and final-expression blocks

## Common early mistakes

- forgetting to import the `std:` module you want to use
- trying to call array prototype methods such as `.map`
- writing `return`
- trying to reassign a binding with `let` or `const`

## Next step

Continue with [Project Layout](./project-layout.md) or jump into the [Language Guide](../language-guide/syntax-overview.md).
