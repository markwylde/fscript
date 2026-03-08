# fscript

## Quick start

Build the project:

```sh
cargo build
```

Run the CLI through Cargo:

```sh
cargo run -p fscript-cli -- --help
```

Note:

- the Cargo package is `fscript-cli`
- the compiled binary is `fscript`

Run the hello world example:

```sh
fscript run examples/hello_world.fs
```

Run the HTTP hello server example:

```sh
cargo run -p fscript-cli -- run examples/http_hello_server/main.fs
```

Compile a supported source file into a native executable:

```sh
cargo run -p fscript-cli -- compile examples/hello_world.fs ./hello-world
./hello-world
```

## Development

Run the full workspace test suite:

```sh
cargo test
```

Run the CLI end-to-end tests only:

```sh
cargo test -p fscript-cli
```

Run snapshot tests managed by `insta`:

```sh
cargo insta test
```

Accept updated snapshots after review:

```sh
cargo insta accept
```

Measure workspace coverage with `cargo-llvm-cov`:

```sh
cargo llvm-cov --workspace --all-features
```

Notes:

- lexer and parser snapshots live alongside their frontend crates and use `insta`
- driver diagnostics and CLI output are snapshot-tested as user-facing surfaces
- CLI integration tests execute the built `fscript` binary end-to-end
- `cargo insta` and `cargo llvm-cov` require their corresponding Cargo subcommands to be installed locally
