# HTTP Hello Server

Run with:

```text
cargo run -p fscript-cli -- run examples/http_hello_server/main.fs
```

Or, after building:

```text
fscript run examples/http_hello_server/main.fs
```

This example uses the new `std:http` host module to start a tiny HTTP server on `127.0.0.1:8080`.

Routes:

- `GET /` returns `hello from fscript`
- `GET /health` returns a small JSON health payload
- every other route returns `404 not found`

Implementation notes:

- `maxRequests: 0` means the server keeps accepting requests until the process is stopped
- the request handler lives in `router.fs`
- response helpers live in `response.fs`
