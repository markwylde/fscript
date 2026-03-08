---
title: Running, Checking, and Compiling
description: The three main CLI workflows for validating source, executing programs, and building native binaries.
---

Use `check` when you want validation, `run` when you want the broadest current execution path, and `compile` when you want a native executable for the supported subset.

## Check

```sh
fscript check examples/hello_world.fs
```

## Run

```sh
fscript run examples/hello_world.fs
```

## Compile

```sh
cargo run -p fscript-cli -- compile examples/hello_world.fs ./hello-world
./hello-world
```

## Important note

`compile` is currently narrower than `run`. If something works in interpreter mode but not the native path yet, that is expected at this stage of the implementation.

