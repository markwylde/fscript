---
title: Running, Checking, and Compiling
description: The three main CLI workflows for validating source, executing programs, and building native binaries.
---

Use `check` when you want validation, `run` when you want the broadest current execution path, and `compile` when you want a native executable for the supported subset.

## `fscript check`

`check` is the safest first step while you are writing code.

It currently validates:

- lexing and parsing
- name resolution
- typechecking
- effect analysis
- user-module import graphs

Example:

```bash
cargo run -p fscript-cli -- check src/main.fs
```

Use this when you want fast feedback without executing effects.

## `fscript run`

`run` executes an entrypoint through the current shared IR and interpreter path.

Example:

```bash
cargo run -p fscript-cli -- run src/main.fs
```

This is the broadest execution path today and is the best default while the native compiler is still expanding feature parity.

## `fscript compile`

`compile` emits a native executable:

```bash
cargo run -p fscript-cli -- compile src/main.fs ./main
```

Important current nuance:

- there is a real native backend slice
- there is also a broader embedded-runner bridge
- `compile` therefore covers more than the narrowest real-native path, but less than the long-term goal of full parity

## Recommended workflow

For day-to-day iteration:

1. write or edit a `.fs` file
2. run `fscript check`
3. run `fscript run`
4. compile only when you need an executable artifact or want to test current compile coverage

## Comparison to TypeScript workflows

If you are used to `tsc --noEmit`, `node`, and bundlers:

- `check` is closest to `tsc --noEmit`
- `run` is the closest equivalent to "execute directly"
- `compile` is closer to producing a native binary than transpiling to JavaScript

## Choosing the right command

Use `check` when:

- you only want validation
- the program has side effects you do not want to trigger yet

Use `run` when:

- you want the most complete current execution path
- you are testing behavior rather than binary output

Use `compile` when:

- you want a standalone executable
- you are explicitly testing compile coverage
- you are comfortable with the current implementation-status caveats

## Next step

See the [CLI Overview](../cli/overview.md) for command-by-command details.
