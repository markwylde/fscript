---
title: Effects
description: The difference between pure and effectful code in FScript and how the runtime handles effectful operations.
---

FScript distinguishes between pure expressions and effectful operations.

## Pure code

- arithmetic
- record construction
- array construction
- local transformations

Pure code evaluates immediately and should avoid scheduler overhead.

## Effectful code

- filesystem access
- network access
- time
- randomness
- process interaction

Effectful calls start eagerly by default and may suspend implicitly when their values are consumed.

