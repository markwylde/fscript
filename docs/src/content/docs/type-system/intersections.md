---
title: Intersection Types
description: Combine compatible type requirements into a single value shape.
---

Intersection types represent values that satisfy multiple type requirements at once.

```fs
type Named = { name: String }
type Active = { active: Boolean }
type ActiveUser = Named & Active
```

They are most useful when composing record-like constraints.

