---
title: Defer and Laziness
description: Use defer when you want laziness explicitly rather than the normal eager effect-start behavior.
---

Effects start eagerly by default. `defer` is how you choose laziness explicitly.

```fs
a = defer getA()
b = defer getB()
```

## Runtime model

- creating a deferred value does not start the effect
- forcing or invoking it starts the effect exactly once
- repeated forcing should reuse the same eventual result in Draft 0.1

## Why this is explicit

FScript wants eagerness to be the default effect model and laziness to be a deliberate choice.

