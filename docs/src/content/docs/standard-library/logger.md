---
title: std:logger
description: Runtime-backed terminal logging for text messages and pretty JSON output.
---

# `std:logger`

`std:logger` provides runtime-backed logging.

```fscript
import Logger from 'std:logger'
```

## Typical usage

```fscript
Logger.info('starting')
Logger.info({ tag: 'config_loaded', path: './config.json' })
Logger.error('failed to load config')
```

## Why logging is a module

FScript does not rely on a global `console` object. Logging is an explicit runtime capability, just like filesystem or task helpers.

## Good use

- emit human-readable progress messages
- log tagged records when structured output is useful
- keep logging at effect boundaries rather than burying it inside pure helpers

## Current implementation note

The current runtime already ships logger support and the getting-started docs use it in the first program examples.
