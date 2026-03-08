---
title: Result-Based Error Handling
description: Prefer typed recoverable errors over exceptions for expected failures.
---

# Result-Based Error Handling

For expected failures, FScript favors `Result<T, E>`.

```fs
type Result<T, E> =
  | { tag: 'ok', value: T }
  | { tag: 'error', error: E }
```

That shape works well with:

- tagged unions
- `match`
- explicit domain modeling

```fs
match (parsed) {
  { tag: 'ok', value } => value
  { tag: 'error', error } => error.message
}
```

## Related Pages

- [Errors](../language-guide/errors.md)
- [Pattern matching](../language-guide/pattern-matching.md)
- [std:result](../standard-library/result.md)
