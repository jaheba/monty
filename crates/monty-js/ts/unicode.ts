/** Reject text that UTF-8 encoders would silently replace. */
export function assertScalarString(text: string): void {
  for (let i = 0; i < text.length; i++) {
    const unit = text.charCodeAt(i)
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = text.charCodeAt(++i)
      if (next >= 0xdc00 && next <= 0xdfff) continue
    } else if (unit < 0xdc00 || unit > 0xdfff) {
      continue
    }
    throw new TypeError('Lone surrogates are unsupported in Monty strings')
  }
}

/** Encoded nodes contain only plain records, arrays and typed buffers. */
export function assertScalarNode(node: unknown): void {
  const pending: unknown[] = [node]
  const seen = new Set<object>()
  while (pending.length > 0) {
    const value = pending.pop()
    if (typeof value === 'string') {
      assertScalarString(value)
    } else if (value !== null && typeof value === 'object' && !ArrayBuffer.isView(value) && !seen.has(value)) {
      seen.add(value)
      for (const child of Object.values(value)) pending.push(child)
    }
  }
}
