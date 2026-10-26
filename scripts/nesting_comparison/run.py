"""Compare the checked-out Monty guards with a local Tokspam checkout."""

import argparse
import hashlib
import json
import subprocess
import sys
import tempfile
from pathlib import Path


def cases():
    shapes = {
        'parentheses': lambda n: '(' * n + 'x' + ')' * n,
        'lists': lambda n: '[' * n + 'x' + ']' * n,
        'unary': lambda n: '-' * n + 'x',
        'not': lambda n: 'not ' * n + 'x',
        'power': lambda n: 'x**' * n + 'x',
        'lambda': lambda n: 'lambda: ' * n + 'x',
        'conditional': lambda n: 'x if x else ' * n + 'x',
        'addition': lambda n: 'x+' * n + 'x',
        'attributes': lambda n: 'x' + '.x' * n,
        'calls': lambda n: 'x' + '()' * n,
        'subscripts': lambda n: 'x' + '[0]' * n,
        'union': lambda n: 'x|' * n + 'x',
        'boolean': lambda n: 'x and ' * n + 'x',
        'comparison': lambda n: 'x<' * n + 'x',
        'siblings': lambda n: '[' + '-x,' * n + ']',
        'indentation': lambda n: ''.join(' ' * i + 'if x:\n' for i in range(n)) + ' ' * n + 'pass\n',
        'annotation_parens': lambda n: "x: '" + '(' * n + 'T' + ')' * n + "'",
        'annotation_union': lambda n: "x: '" + 'T|' * n + "T'",
        'annotation_subscript_union': lambda n: "x: list['" + 'T|' * n + "T']",
        'annotation_nested_union': lambda n: 'x: "list[\'' + 'T|' * n + 'T\']"',
        'nested_calls': lambda n: 'f(' * n + 'x' + ')' * n,
        'left_grouped': lambda n: '(' * n + 'x' + '+x)' * n,
        'mixed_postfix': lambda n: 'x' + '.a()[0]' * n,
        'lambda_parameters': lambda n: 'lambda ' + ','.join(f'a{i}' for i in range(n)) + ': x',
        'comprehension': lambda n: '[x ' + 'for x in y ' * n + ']',
        'ordinary_string': lambda n: "x = '" + '(' * n + 'T' + ')' * n + "'",
        'literal_metadata': lambda n: "x: Literal['" + '(' * n + 'T' + ')' * n + "']",
        'annotated_metadata': lambda n: "x: Annotated[int, '" + '(' * n + 'T' + ')' * n + "']",
        'escaped_annotation': lambda n: "x: '" + r'\x28' * n + 'T' + r'\x29' * n + "'",
        'format_spec': lambda n: "f'" + '{x:' * n + '{x}' + '}' * n + "'",
        'case_pattern': lambda n: 'match x:\n case ' + '1+' * n + '1j:\n  pass\n',
        'recovered_if': lambda n: 'x = 1 ' + 'if ' * n + '1',
        'recovered_if_plus': lambda n: 'x = 1 ' + 'if + ' * n + '1',
        'recovered_if_not': lambda n: 'x = 1 ' + 'if not ' * n + '1',
        'recovered_if_condition': lambda n: 'x = 1 ' + 'if 1 ' * n + '1',
        'recovered_if_comma': lambda n: 'x = 1 ' + 'if , ' * n + '1',
        'recovered_if_colon': lambda n: 'x = 1 ' + 'if : ' * n + '1',
        'recovered_if_await': lambda n: 'x = 1 ' + 'if await ' * n + '1',
        'recovered_if_boolean': lambda n: 'x = 1 ' + 'and if ' * n + '1',
    }
    for shape, generate in shapes.items():
        for n in (0, 1, 2, 10, 29, 30, 31, 98, 99, 100, 101, 198, 199, 200, 201, 202, 250, 500):
            source = generate(n)
            for threshold, padded in ((0, False), (4096, False), (4096, True)):
                text = source + ('\n#' + ' padding' * 520 if padded else '')
                yield {
                    'name': f'{shape}/{n}/{threshold}/{"padded" if padded else "raw"}',
                    'source': text,
                    'threshold': threshold,
                    'iterations': 10,
                }
    for name, source in {
        'named_escape': r"x: '\N{LATIN CAPITAL LETTER A}'",
        'unterminated': "x = '''unfinished",
        'unclosed_bracket': 'x = [1, 2',
        'flat_module': 'x = 1\n' * 2000,
        'nested_annotation_escapes': r'''x: "list['\x28T\x29']"''',
        'annotation_prose': '''x: "it's"''',
        'annotation_raw_escape': r"x: r'raw\d'",
        'annotation_concatenated_raw': r'''x: "a" r"b\n"''',
    }.items():
        yield {'name': name, 'source': source, 'threshold': 0, 'iterations': 30}
    quoted = 'T'
    for depth in range(1, 9):
        quoted = "'" + quoted.replace('\\', '\\\\').replace("'", "\\'") + "'"
        yield {'name': f'quoted_layers/{depth}', 'source': f'x: {quoted}', 'threshold': 0, 'iterations': 10}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--spam', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--corpus', type=Path, action='append', default=[])
    parser.add_argument(
        '--generator-manifest', type=Path, help='Execute trusted corpus generators and verify their recorded SHA-256'
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[2]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    inputs = list(cases())
    for directory in args.corpus:
        for path in sorted(directory.rglob('*.py')):
            if path.stat().st_size <= 100_000:
                source = path.read_text(errors='replace')
                for threshold in (4096, 0):
                    inputs.append(
                        {
                            'name': str(path) + ('#forced' if threshold == 0 else ''),
                            'source': source,
                            'threshold': threshold,
                            'iterations': 3,
                        }
                    )
    if args.generator_manifest:
        manifest = json.loads(args.generator_manifest.read_text())
        for case in manifest['cases']:
            path = args.generator_manifest.resolve().parent / case['generator']
            generated = subprocess.check_output([sys.executable, str(path)], timeout=10)
            if hashlib.sha256(generated).hexdigest() != case['sha256']:
                raise ValueError(f'Generator output changed: {path}')
            for threshold in (4096, 0):
                inputs.append(
                    {
                        'name': 'attack:' + case['name'] + ('#forced' if threshold == 0 else ''),
                        'source': generated.decode(),
                        'threshold': threshold,
                        'iterations': 3,
                    }
                )
    (output / 'inputs.jsonl').write_text(''.join(json.dumps(case) + '\n' for case in inputs))
    production = root / 'crates/monty/src/source_nesting.rs'

    def revision(path):
        return subprocess.check_output(['git', '-C', str(path), 'rev-parse', 'HEAD'], text=True).strip()

    (output / 'metadata.json').write_text(
        json.dumps(
            {
                'monty_revision': revision(root),
                'spam_revision': revision(args.spam.resolve()),
                'spam_diff_sha256': hashlib.sha256(
                    subprocess.check_output(['git', '-C', str(args.spam.resolve()), 'diff', 'HEAD'])
                ).hexdigest(),
                'source_nesting_sha256': hashlib.sha256(production.read_bytes()).hexdigest(),
                'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip(),
                'cases': len(inputs),
                'limit': 200,
            },
            indent=2,
        )
        + '\n'
    )

    # Include the production file directly, so the comparison cannot drift
    # into a separately maintained implementation of Monty's guards.
    with tempfile.TemporaryDirectory(prefix='monty-nesting-') as tmp:
        project = Path(tmp)
        (project / 'src').mkdir()
        manifest = f"""[package]
name = "monty-nesting-comparison"
version = "0.0.0"
edition = "2024"

[dependencies]
spam = {{ package = "tokspam", path = {json.dumps(str(args.spam.resolve()))} }}
ruff_python_ast = "=0.0.14"
ruff_python_parser = "=0.0.14"
ruff_text_size = "=0.0.14"
serde_json = "1"
"""
        (project / 'Cargo.toml').write_text(manifest)
        harness = Path(__file__).with_name('main.rs').read_text()
        (project / 'src/main.rs').write_text(
            f'#[path = {json.dumps(str(production))}]\nmod source_nesting;\n'
            'mod parse { pub const MAX_NESTING_DEPTH: u16 = 200; }\n' + harness
        )
        target = output / 'target'
        subprocess.run(
            [
                'cargo',
                'build',
                '--offline',
                '--release',
                '--manifest-path',
                str(project / 'Cargo.toml'),
                '--target-dir',
                str(target),
            ],
            check=True,
        )
        binary = target / 'release/monty-nesting-comparison'
        with (output / 'results.jsonl').open('w') as results:
            for case in inputs:
                try:
                    run = subprocess.run(
                        [str(binary)], input=json.dumps(case) + '\n', text=True, capture_output=True, timeout=10
                    )
                    row = (
                        json.loads(run.stdout)
                        if run.returncode == 0
                        else {'name': case['name'], 'process_error': run.returncode, 'stderr': run.stderr[-2000:]}
                    )
                except subprocess.TimeoutExpired:
                    row = {'name': case['name'], 'process_error': 'timeout'}
                results.write(json.dumps(row) + '\n')
                results.flush()
        print(f'Compared {len(inputs)} cases; results: {output / "results.jsonl"}')


if __name__ == '__main__':
    main()
