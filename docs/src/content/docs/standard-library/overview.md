---
title: Standard Library Overview
description: Explicit std modules, curried data-last APIs, and runtime-backed host capabilities.
---

# Standard Library Overview

FScript uses explicit `std:` imports instead of prototype methods or a single global utility namespace.

```fs
import Array from 'std:array'
import Object from 'std:object'
import String from 'std:string'
```

## Design Rules

- modules use the reserved `std:` import scheme
- default imports are the primary standard pattern
- functions are curried by default
- transformation APIs are data-last where appropriate
- collection helpers return new values instead of mutating input

## Required Draft 0.1 Modules

- `std:array`
- `std:object`
- `std:string`
- `std:number`
- `std:result`
- `std:json`
- `std:filesystem`
- `std:task`

## A Note About Current Implementation

The specs describe the intended module surfaces. The current runtime-backed implementation exposes a useful subset already, and some modules are narrower than the representative APIs shown in the specs.

Use the per-module pages for the conceptual model, then check [Implementation Status](../implementation-status/supported-features.md) when you need current-shipping details.

## Related Pages

- [Array](./array.md)
- [Result](./result.md)
- [Filesystem](./filesystem.md)
- [Pipes](../language-guide/pipes.md)
