---
title: Literal Types
description: Narrow value-level literals that support precise tags, domain states, and exhaustive matching.
---

Literal values can participate in narrower types.

```fs
status = 'ok' // 'ok'
type Status = 'ok' | 'error'
```

Literal types are especially useful for tags in union types and for precise pattern matching.

