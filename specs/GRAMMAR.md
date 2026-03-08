# FScript Grammar Specification

Status: Draft 0.1

## 1. Goal

Define the surface grammar for FScript.

This document describes the concrete syntax of the language, not its runtime behavior. Where semantics matter, they should match `specs/LANGUAGE.md`, `specs/TYPESYSTEM.md`, and `specs/RUNTIME.md`.

## 2. Grammar Style

The grammar below is intentionally practical rather than mathematically minimal.

It is written in an EBNF-like style:

- `name = ... ;` defines a production
- `|` means alternative
- `*` means zero or more
- `+` means one or more
- `?` means optional
- quoted text is a literal token

Whitespace and comments may appear between tokens unless otherwise stated.

## 3. Lexical Structure

### 3.1 Source file

A FScript source file uses the `.fs` extension.

### 3.2 Comments

Draft 0.1 supports:

```fs
// line comment

/* block comment */
```

### 3.3 Identifiers

```ebnf
identifier = identifier_start , { identifier_continue } ;
identifier_start = letter | "_" ;
identifier_continue = letter | digit | "_" ;
```

Reserved keywords:

- `import`
- `from`
- `export`
- `type`
- `if`
- `else`
- `match`
- `try`
- `catch`
- `throw`
- `defer`
- `yield`
- `true`
- `false`

Reserved built-in type names:

- `Number`
- `String`
- `Boolean`
- `Null`
- `Undefined`
- `Never`
- `Unknown`

### 3.4 Literals

Supported literal kinds:

- number literals
- string literals
- boolean literals
- `Null`
- `Undefined`

Examples:

```fs
1
3.14
'hello'
"world"
true
false
Null
Undefined
```

## 4. Top-Level Grammar

```ebnf
module = { module_item } ;

module_item = import_decl
            | export_decl
            | type_decl
            | binding_decl
            ;
```

## 5. Imports and Exports

### 5.1 Imports

FScript supports default imports and named imports.

```ebnf
import_decl = "import" , import_clause , "from" , string_literal ;

import_clause = identifier
              | "{" , import_name_list? , "}"
              ;

import_name_list = identifier , { "," , identifier } , ","? ;
```

Examples:

```fs
import Array from 'std:array'
import Object from 'std:object'
import FileSystem from 'std:filesystem'
import { parseUser, validateUser } from './user.fs'
```

### 5.2 Exports

```ebnf
export_decl = "export" , ( binding_decl | type_decl ) ;
```

Examples:

```fs
export parseUser = (text: String): User => {
  Json.parse(text)
}

export type User = {
  id: String,
  name: String,
}
```

## 6. Bindings

Bindings are immutable and use plain `=`.

```ebnf
binding_decl = pattern , "=" , expr ;
```

Examples:

```fs
answer = 42
user = { name: 'Ada' }
{name, age} = person
[first, second] = pair
```

Rules:

- binding declarations are immutable
- rebinding the same name in the same scope is a compile error
- `const`, `let`, and `var` are not part of the grammar

## 7. Type Declarations

```ebnf
type_decl = "type" , identifier , type_params? , "=" , type_expr ;

type_params = "<" , identifier , { "," , identifier } , ">" ;
```

Examples:

```fs
type User = {
  id: String,
  name: String,
}

type Maybe<T> = T | Null
```

## 8. Patterns

Patterns are used in bindings, parameters, and `match` arms.

```ebnf
pattern = identifier_pattern
        | record_pattern
        | array_pattern
        | literal_pattern
        ;

identifier_pattern = identifier ;

record_pattern = "{" , record_pattern_fields? , "}" ;
record_pattern_fields = record_pattern_field , { "," , record_pattern_field } , ","? ;
record_pattern_field = identifier
                     | identifier , ":" , pattern
                     ;

array_pattern = "[" , array_pattern_items? , "]" ;
array_pattern_items = pattern , { "," , pattern } , ","? ;

literal_pattern = number_literal
                | string_literal
                | "true"
                | "false"
                | "Null"
                | "Undefined"
                ;
```

Examples:

```fs
{name} = user
{tag: 'member', name} = user
[first, second] = items
```

## 9. Types

```ebnf
type_expr = union_type ;

union_type = intersection_type , { "|" , intersection_type } ;
intersection_type = postfix_type , { "&" , postfix_type } ;

postfix_type = primary_type , array_type_suffix* ;
array_type_suffix = "[" , "]" ;

primary_type = identifier
             | literal_type
             | record_type
             | function_type
             | generic_type
             | "(" , type_expr , ")"
             ;

generic_type = identifier , "<" , type_expr , { "," , type_expr } , ">" ;

record_type = "{" , record_type_fields? , "}" ;
record_type_fields = record_type_field , { "," , record_type_field } , ","? ;
record_type_field = identifier , ":" , type_expr ;

function_type = "(" , function_type_params? , ")" , ":" , type_expr ;
function_type_params = function_type_param , { "," , function_type_param } , ","? ;
function_type_param = identifier , ":" , type_expr ;

literal_type = number_literal | string_literal | "true" | "false" ;
```

Examples:

```fs
type Point = { x: Number, y: Number }
type Maybe<T> = T | Null
type Mapper<T, U> = (value: T): U
type Person = { name: String } & { age: Number }
```

## 10. Expressions

```ebnf
expr = assignment_like_expr ;
```

There is no mutable assignment expression in FScript.

The top-level expression grammar is organized by precedence.

### 10.1 Precedence overview

From lowest precedence to highest:

1. pipe
2. conditional / `if` / `match` / `try`
3. logical OR `||`
4. logical AND `&&`
5. nullish coalescing `??`
6. equality `===` `!==`
7. relational `< <= > >=`
8. additive `+ -`
9. multiplicative `* / %`
10. unary
11. call / member / index
12. primary

### 10.2 Expression productions

```ebnf
assignment_like_expr = pipe_expr ;

pipe_expr = conditional_expr , { "|>" , conditional_expr } ;

conditional_expr = if_expr
                 | match_expr
                 | try_expr
                 | logical_or_expr
                 ;

logical_or_expr = logical_and_expr , { "||" , logical_and_expr } ;
logical_and_expr = nullish_expr , { "&&" , nullish_expr } ;
nullish_expr = equality_expr , { "??" , equality_expr } ;

equality_expr = relational_expr , { ( "===" | "!==" ) , relational_expr } ;
relational_expr = additive_expr , { ( "<" | "<=" | ">" | ">=" ) , additive_expr } ;
additive_expr = multiplicative_expr , { ( "+" | "-" ) , multiplicative_expr } ;
multiplicative_expr = unary_expr , { ( "*" | "/" | "%" ) , unary_expr } ;

unary_expr = ( "!" | "-" | "+" | "defer" ) , unary_expr
           | postfix_expr
           ;

postfix_expr = primary_expr , { postfix_op } ;
postfix_op = call_suffix | member_suffix | index_suffix ;

call_suffix = "(" , arg_list? , ")" ;
arg_list = expr , { "," , expr } , ","? ;

member_suffix = "." , identifier ;
index_suffix = "[" , expr , "]" ;
```

## 11. Primary Expressions

```ebnf
primary_expr = literal
             | identifier
             | record_literal
             | array_literal
             | arrow_function
             | generator_arrow_function
             | block_expr
             | paren_expr
             | throw_expr
             | yield_expr
             ;

paren_expr = "(" , expr , ")" ;
```

## 12. Arrow Functions

Arrow functions are the only function syntax.

```ebnf
arrow_function = function_head , "=>" , function_body ;
generator_arrow_function = "*" , function_head , "=>" , function_body ;

function_head = parameter_list , return_type_annotation? ;
parameter_list = "(" , parameters? , ")" ;
parameters = parameter , { "," , parameter } , ","? ;
parameter = pattern , type_annotation? ;

type_annotation = ":" , type_expr ;
return_type_annotation = ":" , type_expr ;

function_body = expr | block_expr ;
```

Examples:

```fs
add = (a: Number, b: Number): Number => a + b

pair = *(a: Number, b: Number): Sequence<Number> => {
  yield a
  yield b
}
```

## 13. Blocks

Blocks are expressions.

```ebnf
block_expr = "{" , block_items? , "}" ;
block_items = block_item , { block_item } ;
block_item = binding_decl | expr ;
```

Rules:

- block items are evaluated top-to-bottom
- the final expression becomes the block value
- there is no `return` statement in Draft 0.1

Example:

```fs
test = () => {
  a = 1
  a
}
```

## 14. Record and Array Literals

```ebnf
record_literal = "{" , record_literal_fields? , "}" ;
record_literal_fields = record_literal_field , { "," , record_literal_field } , ","? ;
record_literal_field = identifier , ":" , expr ;

array_literal = "[" , array_literal_items? , "]" ;
array_literal_items = expr , { "," , expr } , ","? ;
```

Examples:

```fs
user = { id: '1', name: 'Ada' }
items = [1, 2, 3]
```

Draft 0.1 does not require object spread syntax. Object merging should use `Object.spread(...)` from `std:object`.

## 15. `if` Expressions

```ebnf
if_expr = "if" , "(" , expr , ")" , block_expr , else_clause? ;
else_clause = "else" , ( block_expr | if_expr ) ;
```

When an `if` is used as a value, `else` is required.

Example:

```fs
label = if (count > 0) {
  'active'
} else {
  'empty'
}
```

## 16. `match` Expressions

```ebnf
match_expr = "match" , "(" , expr , ")" , "{" , match_arms? , "}" ;
match_arms = match_arm , { match_arm } ;
match_arm = pattern , "=>" , match_arm_body , ","? ;
match_arm_body = expr | block_expr ;
```

Example:

```fs
label = match (user) {
  { tag: 'guest' } => 'Guest',
  { tag: 'member', name } => name,
}
```

## 17. `try` / `catch` Expressions

```ebnf
try_expr = "try" , block_expr , "catch" , "(" , pattern , ")" , block_expr ;
```

Example:

```fs
value = try {
  readConfig(path)
} catch (error) {
  defaultConfig
}
```

## 18. `throw` and `yield`

```ebnf
throw_expr = "throw" , expr ;
yield_expr = "yield" , expr ;
```

Rules:

- `throw` is an expression of type `Never`
- `yield` is valid only inside a generator arrow body

## 19. Calls, Currying, and Pipe

FScript functions are curried by semantics, but calls use familiar argument-list syntax.

Examples:

```fs
add(1, 2)
add(1)
Array.map((i) => i + 1, [1, 2, 3])
[1, 2, 3] |> Array.map((i) => i + 1)
```

Pipe desugaring rule:

```fs
value |> fn(a, b)
```

means:

```fs
fn(a, b, value)
```

## 20. Unsupported Syntax

Draft 0.1 does not include grammar for:

- `function`
- `class`
- `extends`
- `implements`
- `constructor`
- `interface`
- `enum`
- `namespace`
- `new`
- `this`
- `super`
- `instanceof`
- `var`
- `let`
- `const`
- `return`
- `switch`
- `async`
- `await`
- `require`
- `module.exports`

## 21. Parsing Guidance

Recommended parser strategy:

- recursive descent for module items, declarations, blocks, and structured expressions
- Pratt parsing for operator precedence
- dedicated parse routines for:
  - arrow functions
  - generator arrows
  - destructuring patterns
  - `if`
  - `match`
  - `try/catch`
  - `throw`
  - `yield`
  - `defer`
  - pipe expressions

The parser should support error recovery and continue after local syntax failures.

## 22. Summary

FScript grammar is intentionally small and expression-oriented.

The most important surface rules are:

- immutable bindings via `name = expr`
- arrow functions only
- generator arrows with leading `*`
- no `return`; block value is the final expression
- `if`, `match`, and `try/catch` are expressions
- `defer` is a native unary form
- collection behavior is expressed through imported `std:` modules rather than prototype syntax
