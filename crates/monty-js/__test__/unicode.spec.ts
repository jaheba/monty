import { expect, test } from 'vitest'
import { assertScalarNode, assertScalarString } from '../ts/unicode.js'
import { ArenaEncoder } from '../ts/worker/value.js'

test('Unicode scalar text permits pairs and combining marks', () => {
  for (const text of ['', 'hello', 'a😀é', 'e\u0301', '\ud83d\ude00']) {
    expect(() => assertScalarString(text)).not.toThrow()
    expect(() => new ArenaEncoder().push(text)).not.toThrow()
  }
})

test('host text, nested values, object keys and metadata reject lone surrogates', () => {
  for (const text of ['\ud800', '\udfff', 'a\ud800b', '\ud800\ud800']) {
    expect(() => assertScalarString(text)).toThrow(/surrogate/)
    expect(() => new ArenaEncoder().push(text)).toThrow(/surrogate/)
    expect(() => new ArenaEncoder().push({ text })).toThrow(/surrogate/)
    expect(() => new ArenaEncoder().push({ [text]: 1 })).toThrow(/surrogate/)
    expect(() => assertScalarNode({ name: text })).toThrow(/surrogate/)
  }
})

test('metadata validation does not recurse forever on malformed cyclic fields', () => {
  const value: Record<string, unknown> = {}
  value.self = value
  expect(() => assertScalarNode(value)).not.toThrow()
})
