---
title: fscript compile
description: Compile an entrypoint to a native executable, with important current limitations.
---

# `fscript compile`

`compile` emits a native executable for a supported FScript entrypoint.

## Usage

```sh
fscript compile input.fs output
```

## Example

```sh
cargo run -p fscript-cli -- compile examples/hello_world.fs ./hello-world
./hello-world
```

## Important Caveat

The implementation plan is clear that `compile` is currently narrower than `run`. The docs treat `compile` as real and useful, but not yet at full parity with the broader execution path.

When you are unsure whether a feature is supported end-to-end:

- validate with `check`
- confirm behavior with `run`
- use `compile` for the subset that the backend currently handles

## Related Pages

- [CLI overview](./overview.md)
- [Run](./run.md)
- [Compile vs run](../implementation-status/compile-vs-run.md)
