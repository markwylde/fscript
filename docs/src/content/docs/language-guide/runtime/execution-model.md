---
title: Execution Model
description: How FScript executes pure and effectful code, why effects start eagerly, how implicit suspension works, and what the current runtime implements today.
---

FScript is async by semantics, not by syntax.

That is the best one-line summary of this page.

The source language does not ask you to write `async` and `await`, but effectful work is still first-class in the language model. The compiler and runtime distinguish between pure computation and host-backed work, and they use that distinction to decide what runs immediately, what may suspend, and what can be delayed with `defer`.

## The short version

- pure expressions evaluate immediately
- effectful calls start eagerly when execution reaches them
- values from effectful calls behave like ordinary values in source code
- execution suspends only when a value is needed and not ready yet
- observable effect ordering is preserved unless work is proven independent
- `defer` changes the start moment explicitly

## Pure code versus effectful code

Pure code is direct evaluation over values:

- arithmetic
- record construction
- array transformation
- string processing
- pattern matching
- pure function calls

Effectful code is host-backed work:

- file IO
- network IO
- time
- randomness
- process interaction

The runtime model exists mainly so pure code can stay cheap while effectful work still behaves predictably.

## Why this model exists

FScript wants three things at once:

- sequential-looking source code
- explicit effect semantics
- low overhead for pure paths

In JavaScript and TypeScript, these concerns are often exposed directly through `Promise`, `async`, and `await`. In FScript, the runtime carries more of that machinery so the source language can stay smaller and more direct.

## Example: file read followed by write

```fscript
import FileSystem from 'std:filesystem'

copyFile = (from: String, to: String): String => {
  text = FileSystem.readFile(from)
  FileSystem.writeFile(to, text)
  text
}
```

The source reads like ordinary sequential code, but the runtime interpretation is richer:

1. execution reaches `FileSystem.readFile(from)`
2. the read starts immediately
3. execution can continue until the actual contents of `text` are required
4. if `text` is not ready at that point, execution suspends there
5. once `text` is ready, `FileSystem.writeFile(to, text)` can start
6. the block result is `text`

That combination of eager start plus implicit suspension is one of the defining semantics of the language.

## Eager effect start

Effectful calls begin when reached.

This is a deliberate design choice. FScript could have chosen a lazy-by-default model where effectful calls build suspended work until something explicit triggers them. Instead, the language keeps ordinary effectful calls action-oriented and uses `defer` as the explicit escape hatch for delayed start.

That means:

- `FileSystem.readFile(path)` means "start reading"
- `FileSystem.writeFile(path, text)` means "start writing"
- `defer FileSystem.readFile(path)` means "capture this work, but do not start it yet"

This keeps plain call syntax intuitive and makes laziness visible.

## Implicit suspension

Values from effectful calls are written as though they were ordinary values:

```fscript
content = FileSystem.readFile(path)
size = String.length(content)
```

There is no explicit `await content`.

Instead, the runtime inserts suspension points automatically when a not-yet-ready value is consumed. If `content` is ready already, execution continues directly. If not, execution suspends until the value resolves.

This gives FScript a very specific feeling:

- source code looks sequential
- effectful work can overlap with later computation when dependencies allow
- the runtime, not the user, manages most suspension points

## Observable ordering

FScript is not trying to invent arbitrary concurrency.

The runtime should preserve observable source ordering unless effects are proven independent.

Example:

```fscript
import FileSystem from 'std:filesystem'

appendLine = (path: String, line: String): Undefined => {
  before = FileSystem.readFile(path)
  FileSystem.writeFile(path, before + '\n' + line)
}
```

The write depends on the read result, so the runtime must not reorder those operations.

By contrast, independent effectful calls may be able to overlap:

```fscript
left = FileSystem.readFile(leftPath)
right = FileSystem.readFile(rightPath)
```

Because neither read depends on the other, the runtime may overlap them while still preserving the program's observable meaning.

## Dependency-driven scheduling

The specs allow the compiler or runtime to build a dependency graph from bindings.

Example:

```fscript
a = getA()
b = getB()
c = combine(a, b)
c
```

The intended interpretation is:

- start `getA()` when reached
- start `getB()` when reached
- wait until both are available before `combine(a, b)` can proceed

This is one of the reasons FScript can overlap independent work without making users write explicit promise orchestration everywhere.

## `defer` changes the start moment

`defer` is the way to opt out of eager start.

```fscript
lazyConfig = defer FileSystem.readFile('./config.json')
```

Creating `lazyConfig` does not start the read. The work begins later when the deferred value is forced or invoked through the deferred/task surface.

This is useful when:

- work is optional
- work is expensive
- a branch may never use it
- you want to assemble a plan before triggering effects

Without `defer`, the language assumes that reaching the effectful call is enough to begin the work.

## Example: eager work plus optional deferred work

```fscript
import FileSystem from 'std:filesystem'

loadPair = (mainPath: String, backupPath: String, includeBackup: Boolean) => {
  main = FileSystem.readFile(mainPath)
  backup = defer FileSystem.readFile(backupPath)

  if (includeBackup) {
    {
      main,
      backup,
    }
  } else {
    {
      main,
      backup: Null,
    }
  }
}
```

This example shows the intended split clearly:

- the main read begins right away
- the backup read is delayed because it is optional

## Scheduler model

Draft 0.1 uses a single-threaded scheduler.

That design is motivated by:

- simpler semantics
- easier determinism
- lower implementation complexity
- better alignment with effect-ordering rules

The scheduler is responsible for:

- ready tasks
- suspended tasks
- resuming work when dependencies resolve
- preserving observable ordering
- supporting explicitly deferred work

The scheduler should not invent concurrency where dependency or ordering rules do not allow it.

## Pure code should stay cheap

One of the most important constraints in the specs is that pure code should not pay async scheduler overhead.

That is why the runtime model draws such a strong line between:

- immediate expressions like `1 + 2`
- suspendable operations like `FileSystem.readFile(path)`

Only effectful operations participate in suspension and scheduler-managed work.

## Effect inference and execution

The type/effect story matters here too.

Draft 0.1 expects effect inference and type inference to cooperate:

- a function that calls an effectful function becomes effectful
- pure functions cannot secretly remain typed as pure if they perform effects
- exported APIs should surface inferred effect information in diagnostics and tooling

That matters because execution strategy depends on effect classification. The runtime only needs scheduler machinery where the program actually performs effects.

## Compared with JavaScript and TypeScript

JavaScript and TypeScript usually make async structure explicit in user code:

- create a promise
- mark the function `async`
- `await` the result
- use helpers such as `Promise.all`

FScript keeps the semantics but moves much of the machinery under the language runtime:

- effectful calls start when reached
- consuming results may suspend
- independent work may overlap
- `defer` is the explicit lazy form

That gives the language a more direct surface while still preserving a rigorous runtime model.

## Current implementation notes

The current repository already implements a meaningful slice of this model:

- effect analysis classifies current callables as `Pure`, `Effectful`, or `Deferred`
- `fscript check` validates effect analysis for the supported frontend
- the shared runtime and interpreter support eager start for the currently implemented host operations
- deferred/task state transitions are tracked explicitly in the shared runtime
- `defer` is memoized in the current shared-interpreter/runtime path
- `fscript run` is the most complete execution path today

The main remaining gap is not the basic eager-start model itself. It is the longer-lived dependency-driven scheduler/runtime parity work still needed across the full evaluation lifecycle, especially for the broader compile path.

## Reading guide

If you are learning the model from scratch:

1. read this page first
2. read [Effects](/fscript/language-guide/effects/)
3. read [Defer and Laziness](/fscript/language-guide/effects/defer-and-laziness/)
4. read [Runtime Tasks](/fscript/runtime/tasks/)
5. read [Runtime Overview](/fscript/runtime/overview/)
