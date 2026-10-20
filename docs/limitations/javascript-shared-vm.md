# Experimental shared-VM JavaScript

Enable the Rust `monty/javascript` feature and use `MontyRepl::feed_javascript` or
`feed_javascript_start`. This main-based experiment has no JS worker, client binding,
DOM, filesystem or network API. Host calls are explicit suspensions serviced by the caller.

OXC lowers JS to the shared compiler IR. The same compiler, values, heap, functions,
frames, scheduler, resource checks and snapshot format serve Python and JS. A handwritten
Python compatibility library runs inside that VM; it never executes in the host Python process.
Its names beginning with `_js_` are reserved in JS. The first JS execution initializes globals such as
`JSON` and `Object`, replacing existing bindings with those names. Python can deliberately replace shared globals.

## Supported subset

- `let`/`const`, objects and arrays, property/index reads and writes, destructuring, rest and spread.
- Named function declarations, anonymous function expressions, expression and block arrows,
    defaults, async functions and `await`. Extra arguments are ignored by JS functions.
- Mutable captures across function scopes, including callbacks called from Python.
- `if`, `while`, classic `for`, `for...of`, `break`, `continue`, `return`, `try`/`catch`/`finally`
    and throwing supported exception objects.
- Arithmetic, comparisons, strict equality, logical operators, nullish coalescing,
    simple optional member access/calls, template strings, identifier increments and `typeof`.
- Array `map`, `flatMap`, `filter`, `reduce`, `find`, `findIndex`, `some`, `every`, `forEach`, `push`, `pop`,
    `shift`, `unshift`, `concat`, `join`, `includes`, `indexOf`, `reverse`, `sort`, `flat`, `at` and `slice`.
- String trimming, case conversion, splitting, slicing, searching, replacement and number formatting.
- Common `Object` operations, JSON parsing/stringifying, numeric conversion and `Math` operations.
- Insertion-ordered `Set`/`Map`, object identity keys, `Array.from`, `Array.isArray`, `Promise.all`,
    `Promise.resolve` and `Promise.reject`.

The constructors and methods above are subsets, not full implementations of their ECMAScript APIs.
Array callbacks receive JS arguments, including index and array. Python callbacks must accept the
corresponding arguments; the frontend does not change Python signatures.

## Deliberate differences and gaps

- Strings use Unicode code points: `'😀'.length` is 1. Lone UTF-16 surrogates are rejected.
- `null`, `undefined`, absent properties, array holes and missing arguments share Python's `None`.
    Defaults therefore also apply to explicit `null`; `String(undefined)` returns `'null'`.
    `typeof undefined` is special-cased, but `typeof` on other missing values cannot distinguish them from null.
- Integers, division errors, remainder, non-addition numeric coercion and most error classes follow Python.
    Addition and string interpolation provide JS-style conversion for the supported primitive/container values.
    `==` and `!=` retain Python equality. Strict equality distinguishes booleans from numbers and compares objects by identity.
- JS conditions treat arrays and objects as true even when empty. Python truth tests are unchanged.
- Block bindings are flattened into their function scope. `const` is mutable, there is no temporal dead zone,
    function declarations are not hoisted and loop iterations do not create fresh capture cells.
- Object keys use dictionary insertion order, including integer-looking keys. Prototypes, accessors,
    `this`, classes, named function expressions, generators, modules, regex literals and TypeScript are unsupported.
- Optional chains with additional unguarded hops after a short-circuited hop are incomplete.
    Use an explicit `?.` for each nullable hop.
- Classic-for `continue` inside a `finally` region is rejected. Arbitrary thrown JS values are unsupported.
- Promise callbacks (`then`/`catch`), JS microtask ordering and a browser/Node event loop are absent.
    Awaiting uses Monty's existing coroutine/future scheduler.
- Comparator sorting and the library `Set`/`Map` use interpreted linear scans. They are intended for
    bounded sandbox data processing; time and memory limits still apply.
- JSON replacers/revivers, JS console inspection and edge cases in numeric/string formatting are incomplete.

Source is capped at 64 KiB, 24 delimiter levels and bounded work between statements. Regex/comment syntax,
templates, escapes outside strings and else-if chains retain a stricter whole-feed complexity cap until
the scanner handles their grammar. Unsupported syntax fails during compilation, before any snippet executes.

The 27 example programs in `javascript_agent_cases.rs` exercise intended agent workflows. They do not
establish a 90% compatibility rate; that requires a separate, representative corpus of real agent output.
