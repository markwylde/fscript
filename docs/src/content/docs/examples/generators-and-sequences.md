---
title: Generators and Sequences
description: Use generator arrows to model pure lazy sequences.
---

# Generators and Sequences

Generators are a good fit when you want lazy sequence production without turning the code into an async stream abstraction.

```fs
numbers = *(): Sequence<Number> => {
  yield 1
  yield 2
  yield 3
}
```

## Guidance

- keep generator work pure in Draft 0.1
- use them for sequence logic, not host IO
- pair them with array or sequence-consuming helpers as the language grows

## Related Pages

- [Language generators](../language-guide/generators.md)
- [Runtime generators](../runtime/generators.md)
