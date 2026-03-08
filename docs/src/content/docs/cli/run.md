---
title: fscript run
description: Execute an FScript entrypoint through the current runtime and interpreter path.
---

# `fscript run`

`run` executes an FScript entrypoint.

## Usage

```sh
fscript run path/to/file.fs
```

## Examples

```sh
fscript run examples/hello_world.fs
```

```sh
cargo run -p fscript-cli -- run examples/http_hello_server/main.fs
```

## Notes

- this is the broadest current execution path
- the implementation plan treats `run` as the source of truth while the shared execution path matures
- the CLI prints the final value when the entry module produces one

## Related Pages

- [CLI overview](./overview.md)
- [Compile](./compile.md)
- [Compile vs run](../implementation-status/compile-vs-run.md)
