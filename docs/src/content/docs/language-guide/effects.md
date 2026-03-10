---
title: Effects
description: Pure versus effectful code, eager effect start, implicit suspension, effect inference, and practical patterns for structuring FScript programs.
---

Effects are one of the core ideas in FScript.

If you only learn one runtime concept beyond the syntax, learn this one: FScript distinguishes between pure code and effectful code, and that distinction is part of both the language design and the runtime model.

The short version is:

- pure expressions evaluate immediately
- effectful calls start eagerly when execution reaches them
- source code still looks direct and sequential
- the runtime suspends only when an effectful result is actually needed and not ready yet
- `defer` is the explicit way to opt out of eager start

This is a major difference from JavaScript and TypeScript. FScript does not make you write `async` and `await`, but it also does not pretend effects do not exist. Instead, the compiler and runtime track them for you.

## What counts as an effect

An operation is effectful when it interacts with the outside world or depends on runtime state that is not just local pure computation.

Typical examples include:

- reading a file
- writing a file
- making an HTTP request
- reading time
- generating randomness
- interacting with the process or terminal

Pure work, by contrast, is just computation over values:

- arithmetic
- string building
- record construction
- array transformation
- pattern matching
- calling other pure functions

## Pure code

Pure code:

- has no observable external interaction
- can be reasoned about from its inputs alone
- evaluates immediately
- should not pay scheduler overhead

Example:

```fscript
fullName = (first: String, last: String): String => {
  first + ' ' + last
}
```

This function is pure. It computes a value and returns it. Nothing suspends, nothing is scheduled, and nothing touches the outside world.

Another example:

```fscript
import Array from 'std:array'
import String from 'std:string'

normalizeNames = (rows: { name: String }[]): String[] => {
  rows
    |> Array.map((row) => String.trim(row.name))
    |> Array.filter((name) => name !== '')
}
```

Even though this does multiple steps, it is still pure because every step is just transformation of existing values.

## Effectful code

Effectful code performs host-backed work.

Example:

```fscript
import FileSystem from 'std:filesystem'

readConfig = (path: String): String => {
  FileSystem.readFile(path)
}
```

`FileSystem.readFile` is effectful because it crosses the runtime boundary and talks to the filesystem.

A larger example:

```fscript
import FileSystem from 'std:filesystem'
import Json from 'std:json'

loadPort = (path: String): Number => {
  text = FileSystem.readFile(path)
  config = Json.parse(text)
  config.port
}
```

This function contains both effectful work and pure work:

- `FileSystem.readFile(path)` is effectful
- `Json.parse(text)` is a boundary operation over outside data
- reading `config.port` is ordinary pure value access once `config` exists

## Why FScript treats this so seriously

FScript is designed around three related goals:

- pure code should stay cheap and direct
- effectful code should still read naturally
- the runtime, not user-written promise plumbing, should manage suspension

That is why the specs repeatedly describe FScript as "async by semantics" rather than "async by syntax".

In JavaScript and TypeScript, effectful workflows usually become visible through:

- `Promise`
- `async`
- `await`
- explicit batching helpers such as `Promise.all`

In FScript, those source-level constructs are not the main model. The language keeps the distinction between pure and effectful code, but moves most of the machinery into the compiler and runtime.

## Eager effect start

Effectful calls start eagerly when execution reaches them.

That means this function:

```fscript
import FileSystem from 'std:filesystem'

copyFile = (from: String, to: String): String => {
  text = FileSystem.readFile(from)
  FileSystem.writeFile(to, text)
  text
}
```

is not interpreted as "do nothing until `text` is explicitly awaited". There is no `await`. Instead, the runtime behaves more like this:

1. execution reaches `FileSystem.readFile(from)`
2. the read starts
3. execution continues until it needs the actual value of `text`
4. if the value is not ready, execution suspends there
5. once `text` is ready, the write can start
6. the block returns `text`

This keeps source code sequential while still allowing the runtime to manage effectful work efficiently.

## Implicit suspension

FScript source treats values from effectful calls as if they were ordinary values:

```fscript
content = FileSystem.readFile(path)
size = String.length(content)
```

There is no explicit `await content`.

Instead, the runtime inserts suspension points automatically. If `content` is ready by the time `String.length(content)` needs it, execution continues directly. If not, execution suspends until the read completes.

This is one of the defining language behaviors:

- effectful calls start eagerly
- consumption of their results may suspend
- pure code stays direct

## Observable ordering

FScript is not "start everything immediately and hope for the best".

The runtime is meant to preserve observable source ordering unless effects are proven independent. That matters for code like this:

```fscript
import FileSystem from 'std:filesystem'

updateLog = (path: String): Undefined => {
  before = FileSystem.readFile(path)
  FileSystem.writeFile(path, before + '\nnext line')
}
```

The write depends on the read result, so the runtime must preserve that dependency and order.

The specs also leave room for overlap of independent work when dependencies allow it. For example, if two effectful calls do not depend on each other, the runtime may be able to overlap them without you writing something like `Promise.all(...)`.

## `defer` is the escape hatch

Because ordinary effectful calls start eagerly, FScript needs a way to say "not yet".

That is what `defer` is for.

```fscript
lazyConfig = defer FileSystem.readFile('./config.json')
```

Creating the deferred value does not start the read yet. The work starts only when the deferred value is forced or invoked, depending on the surrounding runtime and helper surface.

Use `defer` when:

- work is optional
- work is expensive and may never be needed
- you want to build a plan before starting it
- you want laziness to be obvious in the source

Do not use `defer` as your default style. The language is designed around eager effect start as the normal case.

## Why FScript chose eager-by-default effects

There is another plausible design for a language like this:

- effectful calls could be lazy by default
- some explicit form like `await` could mean "start this now and suspend here if needed"

That design has real appeal. It can make batching and delayed work feel more automatic. It can also make effectful values behave more like suspended plans than immediate actions.

FScript does not choose that design.

Instead, FScript treats ordinary effectful calls as actions that begin when execution reaches them, and uses `defer` for the smaller set of cases where delayed start is the right semantics.

The main reason is that it keeps the basic meaning of a function call intuitive:

- `writeFile(path, text)` means "start writing the file"
- `readFile(path)` means "start reading the file"
- `defer readFile(path)` means "capture this work, but do not start it yet"

That split keeps two concepts separate:

- starting work
- waiting for a result

If laziness were the default, an explicit `await`-like form would often have to mean both "start this work now" and "block here until the result is available". FScript keeps those concerns apart:

- ordinary effectful calls start eagerly
- consuming the result may suspend
- `defer` changes the start moment explicitly

## What the alternative would feel like

A lazy-by-default design would look more like this:

```fscript
await FileSystem.writeFile(path, text)
person = getPerson()
await FileSystem.writeFile(otherPath, otherText)
person.firstName
```

In that world:

- `getPerson()` would describe work without starting it yet
- the explicit `await` sites would trigger immediate execution
- reading `person.firstName` might also become the moment that `person` finally has to run

That model can work, but it changes the meaning of ordinary call syntax. A plain call stops meaning "do the thing now" and starts meaning "build a suspended computation".

FScript is intentionally built around a different intuition:

- plain effectful calls mean "start this work"
- plain pure calls mean "compute this value"
- `defer` means "delay this effectful work on purpose"

## Why this fits the rest of the language

Eager-by-default effects line up well with the rest of FScript's design:

- source code is meant to read sequentially
- pure code should stay direct and low-overhead
- effectful work should be explicit without requiring promise syntax
- laziness should be visible where it exists

It also makes documentation and API reading simpler. When a user sees an effectful standard-library function call, they can assume it starts when reached unless the source explicitly says otherwise with `defer`.

## Effect inference

Draft 0.1 does not require you to write effect annotations on functions, but the compiler still tracks effect information.

The broad rule is simple:

- if a function only calls pure code, it stays pure
- if a function calls an effectful function, it becomes effectful
- pure functions cannot secretly remain typed as pure while performing effects

Example:

```fscript
trimName = (value: String): String => {
  String.trim(value)
}
```

This stays pure.

```fscript
import FileSystem from 'std:filesystem'

readName = (path: String): String => {
  text = FileSystem.readFile(path)
  String.trim(text)
}
```

This becomes effectful because it calls `FileSystem.readFile`.

That effect information is important for:

- diagnostics
- tooling
- generated docs
- keeping pure paths free of unnecessary runtime machinery

## Pure helper, effectful shell

One of the best design habits in FScript is to keep effectful boundaries thin and move most logic into pure helpers.

Example:

```fscript
import FileSystem from 'std:filesystem'
import Json from 'std:json'
import String from 'std:string'

type User = {
  name: String,
  active: Boolean,
}

parseUserName = (text: String): String => {
  value = Json.parse(text)
  String.trim(value.name)
}

readUserName = (path: String): String => {
  text = FileSystem.readFile(path)
  parseUserName(text)
}
```

This split is useful because:

- the pure helper is easier to test
- the effectful boundary is small and obvious
- most of the program remains ordinary value transformation

## Example: file read followed by pure transformation

```fscript
import FileSystem from 'std:filesystem'
import String from 'std:string'

loadTitle = (path: String): String => {
  text = FileSystem.readFile(path)
  lines = String.split('\n', text)
  firstLine = lines[0]
  String.trim(firstLine)
}
```

How to read this:

- file reading is effectful
- splitting and trimming are pure
- the runtime may suspend while waiting for `text`
- once `text` exists, the remaining work is ordinary pure evaluation

## Example: two independent effectful calls

```fscript
import FileSystem from 'std:filesystem'

loadPair = (leftPath: String, rightPath: String): { left: String, right: String } => {
  left = FileSystem.readFile(leftPath)
  right = FileSystem.readFile(rightPath)

  {
    left,
    right,
  }
}
```

This example matters because it shows the difference between source order and runtime scheduling.

Source order is still clear:

1. reach the left read
2. reach the right read
3. build the result record

But because the reads are independent, the runtime may be able to overlap them. The programmer does not need to write explicit promise orchestration for that possibility.

## Example: optional work with `defer`

```fscript
import FileSystem from 'std:filesystem'

loadReport = (path: String, includeRaw: Boolean): { summary: String, raw: String | Null } => {
  rawText = defer FileSystem.readFile(path)

  summary = 'report requested'

  if (includeRaw) {
    {
      summary,
      raw: rawText,
    }
  } else {
    {
      summary,
      raw: Null,
    }
  }
}
```

The important part here is not the exact final syntax of every force site. The important part is the language-level intent:

- without `defer`, reaching the read would start it immediately
- with `defer`, the read stays delayed unless the code actually needs it

## Effects and generators

Draft 0.1 intentionally keeps generators pure-lazy.

That means this shape is valid:

```fscript
numbers = *(): Sequence<Number> => {
  yield 1
  yield 2
  yield 3
}
```

But yielding effectful work from a generator is a type/effect error in Draft 0.1.

That restriction exists because FScript does not want generators to become a confusing second async model. Effects, deferred tasks, and generators each have their own job:

- generators: pure lazy sequences
- ordinary effectful calls: eager-start host work
- `defer`: explicit laziness for effectful work

## Effects and type boundaries

Effects matter especially at boundaries where outside data enters the program.

Examples:

- reading text from a file
- parsing JSON
- future native interop

At those boundaries, values may need validation before the rest of the program can treat them as trusted typed data.

That is why a lot of real FScript code will naturally follow this pattern:

1. perform an effectful operation to get outside data
2. validate or decode it
3. continue with pure typed logic

## Compared with JavaScript and TypeScript

This table is a useful mental shortcut:

- JavaScript/TypeScript: effectful workflows are usually spelled with `Promise`, `async`, and `await`
- FScript: effectful workflows are tracked semantically by the compiler and runtime
- JavaScript/TypeScript: laziness versus eagerness often depends on API design
- FScript: eager start is the default, `defer` is the explicit lazy form
- JavaScript/TypeScript: concurrency often requires explicit batching helpers
- FScript: the runtime may overlap independent work while preserving observable ordering rules

## Good habits

- keep pure logic separate from host interaction
- keep effectful boundaries small
- use `Result` for expected failures at boundaries
- use `defer` only when you really want delayed work
- remember that a function becomes effectful if it calls effectful code

## Common mistakes

- assuming effectful work is lazy by default
- treating `defer` as the normal form instead of the exception
- mixing parsing, validation, file IO, and business logic into one giant effectful function
- trying to use generators as async streams

## Current implementation notes

The current repository already has a meaningful effect-analysis and runtime slice:

- the compiler classifies current callables as `Pure`, `Effectful`, or `Deferred`
- `fscript check` validates effect analysis for the supported frontend
- the shared runtime and interpreter already support eager ordinary effect start for implemented host operations
- deferred work is memoized in the current runtime
- effectful generator work is rejected in the current implementation

The main remaining gap is not the basic effect model itself. It is the longer-lived scheduler/runtime parity work described in the implementation plan.

## Where to go next

- read [Defer and Laziness](/fscript/language-guide/effects/defer-and-laziness/) for the lazy side of the model
- read [Execution Model](/fscript/language-guide/runtime/execution-model/) for the runtime view
- read [Tasks](/fscript/runtime/tasks/) for the scheduler-facing runtime surface
- read [Errors](/fscript/language-guide/errors/) and [std:result](/fscript/standard-library/result/) for boundary-friendly failure handling
