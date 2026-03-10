---
title: Defer and Laziness
description: Why defer exists, how explicit laziness changes effect start, what forcing means, and how deferred work behaves in the current FScript runtime.
---

FScript effects are eager by default.

That means an effectful call normally starts when execution reaches it. `defer` is the language feature that changes that rule intentionally.

```fscript
configText = defer FileSystem.readFile('./config.json')
```

Creating `configText` does not start the file read yet. It captures the work so the runtime can start it later when the deferred value is forced or otherwise consumed through the deferred-task surface.

## The core idea

Without `defer`:

```fscript
text = FileSystem.readFile(path)
```

the read starts immediately when execution reaches the call.

With `defer`:

```fscript
text = defer FileSystem.readFile(path)
```

the read does not start yet.

That difference sounds small, but it has real consequences:

- it changes when IO begins
- it changes whether unused work happens at all
- it makes laziness visible in the source
- it gives the runtime a way to represent delayed effectful work explicitly

## Why laziness is explicit in FScript

FScript deliberately does not make effectful calls lazy by default.

Instead, the language chooses this split:

- ordinary effectful calls mean "start this work"
- `defer` means "capture this work, but do not start it yet"

That keeps plain call syntax intuitive. If you see `FileSystem.readFile(path)`, you can read it as an action that begins when reached. If you see `defer FileSystem.readFile(path)`, you can immediately tell that the code is opting into delayed start.

This fits the wider design of the language:

- source code should still read sequentially
- pure code should stay direct and low-overhead
- effectful work should be explicit without needing `async` / `await`
- laziness should be visible where it exists, not hidden in the default call model

## What `defer` is for

`defer` is useful when work is optional, expensive, or dependent on a later decision.

Common good uses:

- fallback work that may never be needed
- secondary IO that depends on a branch
- building a plan before deciding which effects to trigger
- exposing delayed work as part of an API

Example:

```fscript
import FileSystem from 'std:filesystem'

loadConfig = (mainPath: String, fallbackPath: String, useFallback: Boolean): String => {
  fallback = defer FileSystem.readFile(fallbackPath)
  main = FileSystem.readFile(mainPath)

  if (useFallback) {
    fallback
  } else {
    main
  }
}
```

The intent is:

- the main config should start immediately
- the fallback config should not start unless the branch actually uses it

## What `defer` is not for

`defer` is not the normal way to express effectful work.

Do not wrap all effects in `defer` just because they are effectful. If work should begin when reached, plain calls are the intended style.

`defer` is also not a replacement for:

- generators
- streams
- ordinary function composition

Those solve different problems.

## Deferred work versus generator laziness

Generators are lazy too, but they are a different kind of laziness.

Generator laziness:

```fscript
numbers = *(): Sequence<Number> => {
  yield 1
  yield 2
  yield 3
}
```

Deferred-effect laziness:

```fscript
text = defer FileSystem.readFile(path)
```

In Draft 0.1, generators are for pure lazy sequences. Deferred effects are for delayed host work. The type/effect rules intentionally keep those roles separate, which is why effectful generator work is rejected.

## Runtime model

The runtime treats `defer` as a real runtime-visible construct.

The intended Draft 0.1 behavior is:

- `defer expr` captures the expression and its environment safely
- creating the deferred value does not start the effect
- forcing or invoking the deferred value starts the work
- repeated forcing should observe the same eventual result

The specs prefer memoized single-start semantics.

That means a deferred file read should not accidentally turn into repeated independent reads each time some part of the program touches it. Once started, later force sites should share the same eventual result.

## Why memoization matters

Memoization is what makes `defer` useful as a semantic tool instead of just being "a function to call later".

If deferred work restarted every time, it would be easy to trigger duplicated effects:

- multiple file reads
- multiple HTTP calls
- multiple writes

The preferred model is:

1. capture once
2. start once
3. reuse the same eventual outcome

That is both easier to reason about and more aligned with the design goal of predictable effect behavior.

## Forcing

The docs talk about forcing because that is the semantic event that matters:

- before forcing, the effect has not started
- forcing triggers the deferred work
- once completed, later uses observe the same memoized result

In the current implementation, the runtime-backed task surface includes helpers such as `Task.defer` and `Task.force`. Those are useful when reading implementation-focused examples, but the language concept is bigger than one module API: `defer` is part of the language execution model.

## Example: optional expensive work

```fscript
import FileSystem from 'std:filesystem'

loadReport = (summaryPath: String, rawPath: String, includeRaw: Boolean) => {
  summary = FileSystem.readFile(summaryPath)
  raw = defer FileSystem.readFile(rawPath)

  if (includeRaw) {
    {
      summary,
      raw,
    }
  } else {
    {
      summary,
      raw: Null,
    }
  }
}
```

The summary starts eagerly because the program definitely needs it. The raw content is delayed because it depends on a branch.

## Example: planning work before committing to it

```fscript
import FileSystem from 'std:filesystem'

buildPlan = (configPath: String, secretsPath: String) => {
  config = defer FileSystem.readFile(configPath)
  secrets = defer FileSystem.readFile(secretsPath)

  {
    config,
    secrets,
  }
}
```

This is a good pattern when you want to assemble a plan and then decide later which work should actually begin.

## Compared with JavaScript and TypeScript

JavaScript often mixes eager and lazy behavior depending on the API:

- some promise-returning functions start immediately
- callbacks are often lazy
- frameworks add their own task models
- "when does this actually start?" can depend on conventions rather than language rules

FScript narrows that model:

- ordinary effectful calls start eagerly
- `defer` is the explicit lazy form
- consuming a result may suspend
- laziness is a visible source-level choice

That makes delayed work easier to teach and easier to spot when reading code.

## Good habits

- use plain effectful calls by default
- use `defer` only when delayed start is meaningful
- keep deferred work near the branch or API that justifies it
- prefer small effectful shells with mostly pure transformation logic around them

## Common mistakes

- treating `defer` as the default style for all effects
- assuming laziness is automatic
- using deferred work when the result is definitely needed immediately
- confusing deferred effects with generator-based lazy sequences

## Current implementation notes

The current repository already has a meaningful `defer` slice implemented:

- effect analysis classifies current callables as `Pure`, `Effectful`, or `Deferred`
- the shared interpreter supports lazy memoized `defer`
- the shared runtime tracks deferred/task state transitions such as `created`, `ready`, `running`, `waiting`, `completed`, and `failed`
- `std:task` currently includes `Task.defer` and `Task.force`
- ordinary implemented host operations already start eagerly when reached unless explicitly deferred

The main remaining runtime gap is not the basic `defer` semantics. It is broader scheduler parity and longer-lived dependency draining across whole evaluations.

## Related pages

- [Effects](/fscript/language-guide/effects/)
- [Language Guide Defer and Laziness](/fscript/language-guide/defer-and-laziness/)
- [Execution Model](/fscript/language-guide/runtime/execution-model/)
- [Tasks](/fscript/runtime/tasks/)
- [Generators](/fscript/language-guide/generators/)
