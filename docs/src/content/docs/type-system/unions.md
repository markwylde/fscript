---
title: Union Types
description: Model values that may be one of several alternatives and narrow them through control flow and match.
---

Union types represent values that may be one of several alternatives.

```fs
type Status = 'ok' | 'error'
```

Unions are especially useful with literal tags, optional-like shapes, and validation flows.

