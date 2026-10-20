# monty-execution

Language-independent execution mechanisms used by the Python and JavaScript interpreters.
Both interpreters use its fixed-width operand fetching and frame/handler stack cleanup.
Cleanup takes a language-owned value release callback and preserves each caller's release order.
Value representations, frame layouts and language semantics remain in each interpreter.

See [the extraction audit](../../design/shared-execution.md) for invariants and remaining work.
