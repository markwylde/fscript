# FScript Documentation Plan

Status: Draft 0.1

## 1. Goal

Plan a documentation website for FScript that is:

- more cohesive and approachable than reference-only language docs
- strong enough to onboard new users from zero to productive
- grounded in the existing specifications and current implementation
- written as Markdown content that an Astro docs project can consume later

This document plans the docs site only.
It does not create `./docs/content` yet and it does not build the Astro project yet.

## 2. Source Material

The future docs site should be derived primarily from:

- [LANGUAGE.md](/Users/markwylde/Documents/Projects/fscript/specs/LANGUAGE.md)
- [GRAMMAR.md](/Users/markwylde/Documents/Projects/fscript/specs/GRAMMAR.md)
- [TYPESYSTEM.md](/Users/markwylde/Documents/Projects/fscript/specs/TYPESYSTEM.md)
- [RUNTIME.md](/Users/markwylde/Documents/Projects/fscript/specs/RUNTIME.md)
- [STDLIB.md](/Users/markwylde/Documents/Projects/fscript/specs/STDLIB.md)
- [IMPLEMENTATION_PLAN.md](/Users/markwylde/Documents/Projects/fscript/specs/IMPLEMENTATION_PLAN.md)

The docs should also reflect the currently implemented CLI surface:

- `fscript check`
- `fscript run`
- `fscript compile`

## 3. Documentation Principles

The site should be better than a raw spec dump.

That means:

- teach concepts before overwhelming readers with exhaustive rules
- separate tutorial content from reference content
- make the language feel intentional, not like "JavaScript minus things"
- explain both "what the feature is" and "why FScript does it this way"
- use runnable, compact examples everywhere
- clearly distinguish between language design, current implementation status, and future intent

The site should feel closer to:

- a polished language handbook
- a practical CLI guide
- a trustworthy reference

And less like:

- an internal design memo
- a parser spec copied into web pages
- a changelog masquerading as documentation

## 4. Core Positioning

The docs should present FScript as:

- a small functional typed language
- influenced by JavaScript and TypeScript syntax where it helps familiarity
- explicitly not a JavaScript compatibility layer
- async-by-semantics, immutable, curried, and expression-oriented
- implemented as a native Rust toolchain and runtime

Important messaging to repeat carefully:

- no classes, prototypes, or `this`
- no `async` / `await` in source code
- effects start eagerly by default
- laziness is explicit through `defer`
- standard-library functionality lives in `std:` modules, not prototypes
- not every spec feature necessarily has full implementation parity yet

## 5. Audience

The site should support three reader modes:

### 5.1 New user

Needs:

- what FScript is
- why it exists
- how to run a file
- how syntax differs from JavaScript/TypeScript
- a clear first tour of the language

### 5.2 Working user

Needs:

- a reliable handbook for day-to-day syntax and APIs
- CLI command docs
- standard library reference
- examples of common patterns

### 5.3 Language implementer or contributor

Needs:

- spec-aligned reference pages
- runtime model explanation
- current implementation status and boundaries
- links back to the source specs

## 6. Site Information Architecture

The eventual docs site should be organized into these top-level sections:

1. Introduction
2. Getting Started
3. Language Guide
4. Type System
5. Standard Library
6. CLI
7. Runtime and Execution Model
8. Examples and Patterns
9. Reference
10. Implementation Status

This structure is important.
It keeps narrative teaching separate from exact reference material while still making both easy to find.

## 7. Proposed `docs/content` Structure

The future content tree should look roughly like this:

```text
docs/content/
  index.md
  introduction/
    overview.md
    design-goals.md
    differences-from-javascript.md
  getting-started/
    installation.md
    your-first-program.md
    project-layout.md
    running-checking-compiling.md
  language-guide/
    syntax-overview.md
    modules.md
    bindings-and-immutability.md
    functions.md
    currying-and-partial-application.md
    blocks-and-expressions.md
    records-and-arrays.md
    destructuring.md
    control-flow.md
    pattern-matching.md
    generators.md
    pipes.md
    errors.md
    defer-and-laziness.md
    effects.md
  type-system/
    overview.md
    inference.md
    primitive-types.md
    records.md
    arrays.md
    functions.md
    unions.md
    intersections.md
    literal-types.md
    tagged-unions.md
    generics.md
    narrowing-and-exhaustiveness.md
    unknown-never-null-undefined.md
  standard-library/
    overview.md
    array.md
    object.md
    string.md
    number.md
    result.md
    json.md
    logger.md
    filesystem.md
    task.md
  cli/
    overview.md
    check.md
    run.md
    compile.md
  runtime/
    overview.md
    execution-model.md
    scheduler.md
    tasks.md
    generators.md
    values-and-equality.md
    module-loading.md
    errors-and-boundaries.md
  examples/
    overview.md
    data-pipelines.md
    parsing-and-validation.md
    filesystem-scripts.md
    result-based-error-handling.md
    generators-and-sequences.md
  reference/
    syntax-reference.md
    grammar-reference.md
    operators.md
    keywords.md
    built-in-types.md
    stdlib-index.md
  implementation-status/
    overview.md
    supported-features.md
    compile-vs-run.md
    roadmap.md
```

The exact filenames can change later, but the section split should remain stable.

## 8. Page-by-Page Content Intent

### 8.1 `index.md`

Purpose:

- communicate what FScript is in one screen
- show the shortest compelling example
- explain the core ideas
- route readers into either Getting Started, Language Guide, or CLI docs

Hero topics:

- functional
- typed
- immutable
- async-by-semantics
- native toolchain

### 8.2 Introduction section

This section should answer:

- why FScript exists
- what it keeps from JavaScript/TypeScript
- what it removes
- what mental model readers need before they touch syntax

The "Differences from JavaScript" page should be one of the strongest pages on the site.
It should explicitly call out:

- no classes
- no prototype methods
- no `let` / `const` / `var`
- no `return`
- no `async` / `await`
- no mutation
- no CommonJS

### 8.3 Getting Started section

This section should be practical and task-oriented.

It should cover:

- how to install or build the toolchain
- how to create a `.fs` file
- how to run `fscript check`
- how to run `fscript run`
- how to use `fscript compile`
- what command behavior is implemented today versus still evolving

This section should avoid pretending the language is more finished than it is.

### 8.4 Language Guide section

This is the main handbook.

Each page should follow a consistent shape:

1. short explanation of the concept
2. simple example
3. important rules
4. common patterns
5. related concepts

This section should teach the language in a human order, not in parser-production order.

Recommended order:

1. modules
2. bindings
3. functions
4. currying
5. expressions and blocks
6. records and arrays
7. destructuring
8. conditionals
9. pattern matching
10. generators
11. pipes
12. error handling
13. effects and `defer`

### 8.5 Type System section

This section should be partly tutorial and partly reference.

It should explain:

- what gets inferred
- where annotations are expected
- how structural typing works
- how tagged unions and `match` work together
- how `Unknown`, `Never`, `Null`, and `Undefined` behave
- how soundness goals shape the language

This section should avoid TypeScript jargon when FScript semantics differ.

### 8.6 Standard Library section

This should be one of the most practical parts of the site.

Important framing:

- FScript does not rely on prototype methods
- `std:` modules are explicit imports
- APIs are curried and data-last where appropriate

Every stdlib module page should include:

- why the module exists
- import form
- representative API surface
- examples
- notes on immutability or effect boundaries

### 8.7 CLI section

The CLI docs should be concise, accurate, and implementation-aware.

Pages should document:

- `fscript check`
- `fscript run`
- `fscript compile`

Each command page should include:

- what the command does
- expected arguments
- examples
- current caveats
- relationship to the implementation status page where relevant

Important truth to preserve:

- `compile` is currently narrower than `run`

That distinction should be stated clearly anywhere compile behavior is discussed.

### 8.8 Runtime section

This section should explain the execution model in plain English before using implementation terminology.

Topics:

- eager effect start
- implicit suspension when values are consumed
- single-threaded scheduler
- `defer`
- task model
- generators as lazy sequences
- structural equality
- module initialization
- runtime boundaries and host-backed stdlib behavior

This section should bridge spec ideas to runtime intuition, not dump internal jargon.

### 8.9 Examples section

This section should be pattern-oriented rather than feature-oriented.

Examples should show:

- collection transforms with pipes
- parsing plus validation
- `Result`-based flows
- reading and writing files
- generator-based sequence work
- building values immutably

### 8.10 Reference section

This is where exactness wins over narrative.

It should include:

- syntax reference
- grammar summary
- keywords
- built-in type names
- operator behavior
- stdlib index

This is where advanced users go when they need precision fast.

### 8.11 Implementation Status section

This section should exist from day one so the rest of the site can stay clean.

It should answer:

- what is specified
- what is implemented
- where `run` and `compile` differ today
- what parts are draft or evolving

This page prevents the handbook from becoming misleading.

## 9. Content Rules for the Future Docs

When writing the actual Markdown files later, follow these rules:

- every page must start with a short summary paragraph
- every concept page should contain at least one FScript example
- examples should be small enough to scan quickly
- reference pages should link back to handbook pages for explanation
- handbook pages should link to reference pages for exact rules
- use the same terminology consistently across pages
- do not claim implementation support unless it is known to exist
- when behavior is specified but not fully implemented, say so plainly

## 10. Voice and Writing Style

The future docs should feel:

- confident
- clear
- modern
- technically serious
- friendly without being chatty

Preferred style:

- short paragraphs
- focused headings
- examples first
- plain language before formal terminology

Avoid:

- marketing fluff
- passive voice everywhere
- spec language pasted verbatim without interpretation
- unexplained compiler jargon on beginner pages

## 11. Frontmatter Conventions

The actual Astro-readable Markdown files should likely use frontmatter similar to:

```md
---
title: Functions
description: How FScript functions, currying, and block expressions work.
---
```

Recommended frontmatter fields:

- `title`
- `description`

Possible future fields if the Astro setup wants them:

- `sidebar`
- `tableOfContents`
- `prev`
- `next`

For now, the content plan should assume minimal frontmatter only.

## 12. Documentation Sequencing Plan

When the real docs writing starts, create content in this order:

1. landing page and intro pages
2. getting started pages
3. core language guide pages
4. CLI pages
5. standard library overview plus key modules
6. type system pages
7. runtime pages
8. examples pages
9. reference pages
10. implementation status pages and final cross-linking pass

Reason:

- readers need the guided path first
- CLI and core language pages unlock immediate usefulness
- reference pages are easier to write once the narrative structure is stable

## 13. Cross-Linking Plan

The docs should be heavily cross-linked.

Examples:

- functions page links to currying, types, and pipes
- `match` page links to tagged unions and exhaustiveness
- `Result` page links to error handling and pattern matching
- `defer` page links to runtime execution model and tasks
- CLI compile page links to implementation status
- stdlib pages link to language-guide pages that use those modules

Cross-linking should make the site feel like one system, not a pile of isolated pages.

## 14. Explicit Gaps and Caveats to Preserve

The plan for the docs must preserve these truths:

- the language specs are Draft 0.1
- implementation status is still evolving
- `run` currently covers more than `compile`
- some runtime and scheduler semantics are specified more fully than they are implemented today
- docs must distinguish specification, recommendation, and current behavior

Recommended wording pattern:

- "FScript specifies ..."
- "In the current implementation ..."
- "Draft 0.1 plans ..."

This wording will prevent ambiguity.

## 15. What the Future Docs Should Not Do

The future site should not:

- mirror the raw spec documents one-to-one without adaptation
- bury the CLI inside implementation internals
- present draft features as fully shipped without qualification
- over-focus on Rust internals in user-facing pages
- rely on Node.js terminology when FScript intentionally diverges from Node.js
- assume readers already understand functional programming vocabulary

## 16. Deliverable for the Next Docs Phase

The next implementation step after this plan should be:

1. create `docs/content`
2. create the Markdown files for the planned structure
3. write the highest-priority pages first
4. keep wording aligned with the specs and current CLI behavior
5. leave Astro project wiring for a later step

## 17. Success Criteria

The docs effort should be considered successful when:

- a new reader can understand what FScript is from the landing page alone
- a new user can write and run a small `.fs` file after the Getting Started section
- the language guide covers the major language features with examples
- the standard library pages explain the `std:` model clearly
- the CLI docs are accurate for `check`, `run`, and `compile`
- the site clearly separates spec intent from current implementation status
- the Markdown content is ready to drop into an Astro docs project with minimal restructuring
