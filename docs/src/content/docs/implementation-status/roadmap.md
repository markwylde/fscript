---
title: Roadmap
description: The main near-term direction implied by the implementation plan and current docs.
---

# Roadmap

The implementation plan points to two major near-term tracks.

## 1. Keep `run` as the semantic source of truth

The shared IR, runtime, and interpreter path is already valuable and should keep getting stronger. In practice that means:

- preserving broad frontend coverage
- improving runtime fidelity
- extending scheduler behavior for longer-lived dependency-driven execution

## 2. Expand true native compile coverage

The real native backend already exists for a bounded slice. The next job is to widen that slice until compile parity becomes much closer to `run`.

That includes:

- broadening Cranelift lowering
- deepening the runtime ABI contract
- retiring embedded-runner dependence over time

## 3. Keep the docs honest

During this stage, a good roadmap is also a documentation goal:

- say clearly what is implemented
- distinguish design intent from shipped behavior
- avoid implying full parity where it does not exist yet

## What this means for users today

- learn the language model confidently
- use `check` and `run` as the most stable daily workflow
- treat `compile` as actively improving rather than frozen
