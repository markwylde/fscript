# FScript Native ABI

Status: Draft 0.1

This document defines the stable runtime boundary between generated native code and `fscript-runtime`.

## 1. Goals

The native ABI must:

- preserve the same value semantics as `fscript run`
- keep ownership in `fscript-runtime`, not in generated code
- support a simple first implementation that can grow without breaking the boundary

## 2. Stable Boundary

Draft 0.1 uses a runtime-owned opaque handle as the stable ABI representation for every value that crosses between generated code and `fscript-runtime`.

That includes:

- primitives
- strings
- arrays
- records
- closures
- generators
- deferred handles
- task handles
- native stdlib function values

Generated code may use narrower internal representations for hot pure paths, but once a value crosses the runtime boundary it must be reified as a runtime-owned handle.

## 3. Value Strategy

The chosen representation strategy is hybrid:

- stable ABI: opaque runtime-owned handles for every boundary-crossing value
- optimized internal lowering: unboxed primitives are allowed inside generated code when they do not escape the current compiled frame

Draft 0.1 optimized internal forms:

- `Number`: unboxed `f64`
- `Boolean`: unboxed boolean
- `Null`: immediate tag
- `Undefined`: immediate tag

All other categories remain opaque runtime-owned handles in both the stable ABI and current planned optimized form.

## 4. Ownership

Ownership rules:

- generated code never frees or mutates runtime values directly
- `fscript-runtime` owns allocation and destruction of handle-backed values
- closures, generators, deferred handles, and task handles must be safe to outlive the stack frame that created them

This keeps lifetime and scheduler invariants in one place and avoids backend-specific memory-management rules.

## 5. Current Contract Surface

The current repository exposes the contract in shared metadata first:

- `fscript-runtime::NativeAbiValueKind`
- `fscript-runtime::NativeAbiStorage`
- `fscript-runtime::NativeAbiOwnership`
- `fscript-runtime::NativeAbiValueSpec`

That metadata is the source of truth for expanding the real native backend.

## 6. Follow-up Work

This document intentionally does not claim that the full native backend already uses the handle ABI for all compiled programs.

Remaining implementation work:

- replace the numeric-only print shim with runtime-owned value constructors, printers, and drops
- route native stdlib calls through runtime ABI entrypoints
- lower modules, closures, generators, and scheduler-aware values through the same handle contract
