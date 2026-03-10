---
title: Errors and Boundaries
description: Runtime boundaries, host capabilities, and where validation still matters.
---

# Errors and Boundaries

Well-typed FScript code aims to avoid internal type mismatches at runtime, but host boundaries still matter.

## Common boundary points

- file input
- JSON parsing
- future native interop
- runtime-backed host capabilities

## Why validation still matters

The typechecker can protect internal code only after external values have been parsed or validated into trusted shapes. That is why docs throughout the site keep recommending explicit parsing and `Result`-based validation at boundaries.

## Runtime responsibility

The runtime is responsible for propagating errors across those boundaries clearly without collapsing the distinction between expected failures and exceptional failures.
