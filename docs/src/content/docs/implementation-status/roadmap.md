---
title: Roadmap
description: The main near-term direction implied by the implementation plan and current docs.
---

# Roadmap

The implementation plan points toward a clear next phase rather than an open-ended wishlist.

## Near-Term Direction

- deepen the shared IR and interpreter coverage where it is still conservative
- add scheduler-backed effect execution and host IO semantics
- keep `run` as the source of truth for execution behavior
- grow native code generation toward parity with that shared runtime contract

## Docs Implication

As the implementation evolves, the docs should keep separating:

- specified language behavior
- current implementation behavior
- future planned work

That clarity is part of the product, not just a note for contributors.

## Related Pages

- [Implementation status overview](./overview.md)
- [Supported features](./supported-features.md)
