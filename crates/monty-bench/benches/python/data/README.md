# English subtitle search fixture

`rebar/en-sampled.txt` is the unmodified English OpenSubtitles sample from
[BurntSushi/rebar](https://github.com/BurntSushi/rebar), commit `463d00f31887e84c38467805b9e3122c314b9521`,
path `benchmarks/haystacks/opensubtitles/en-sampled.txt`.
Rebar's `UNLICENSE` and its corpus-provenance README are retained alongside the sample.
The latter attributes the subtitle text to OPUS OpenSubtitles v2018.

The file contains 899,232 UTF-8 bytes and 898,664 characters, including 422 non-ASCII characters (0.047%).
It exercises Unicode handling in otherwise mostly ASCII English text.
Git preserves its bytes, and formatting hooks exclude this pinned fixture.

SHA-256: `0d40805f6d02c8fe02bd75945b98911891f707e8ecb939e018446858065d76ea`.

`../rebar_sherlock_en.py` uses Rebar's `Sherlock Holmes` pattern and its expected count of 513 matches from
`benchmarks/definitions/curated/01-literal.toml`.
Our adaptation uses successive `str.find()` calls and takes a 48-character excerpt at each match.
It verifies the match count, a character-position checksum, `count()` and the final `rfind()` position.
This measures Python string searching and slicing, rather than regex-engine throughput.
