from typing import Final, LiteralString

ascii_letters: Final[LiteralString]
ascii_lowercase: Final[LiteralString]
ascii_uppercase: Final[LiteralString]
digits: Final[LiteralString]
hexdigits: Final[LiteralString]
octdigits: Final[LiteralString]
printable: Final[LiteralString]
punctuation: Final[LiteralString]
whitespace: Final[LiteralString]

def capwords(s: str, sep: str | None = None) -> str: ...
