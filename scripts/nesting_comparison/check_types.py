"""Run Tokspam-accepted generated and attack sources through the actual type checker."""

import argparse
import hashlib
import json
import subprocess
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    parser.add_argument('--online', action='store_true', help='Allow Cargo to fetch missing dependencies')
    parser.add_argument('--target-dir', type=Path)
    parser.add_argument('--reuse', type=Path, help='Reuse identical-source results from an earlier run')
    args = parser.parse_args()
    directory = args.directory.resolve()
    root = Path(__file__).resolve().parents[2]
    results = {row['name']: row for row in map(json.loads, (directory / 'results.jsonl').read_text().splitlines())}
    inputs = list(map(json.loads, (directory / 'inputs.jsonl').read_text().splitlines()))
    selected = {}
    cached = {}
    if args.reuse:
        previous_inputs = {
            row['name']: row['source'].split('\n# padding')[0]
            for row in map(json.loads, (args.reuse / 'inputs.jsonl').read_text().splitlines())
        }
        for row in map(json.loads, (args.reuse / 'type-results.jsonl').read_text().splitlines()):
            key = hashlib.sha256(previous_inputs[row['name']].encode()).hexdigest()
            cached[key] = {key: value for key, value in row.items() if key != 'name'}
    for row in inputs:
        result = results[row['name']]
        if 'test_cases/' in row['name'] or not result.get('spam_type_ok'):
            continue
        # Forced/default policies and padding variants share the same structure.
        source = row['source'].split('\n# padding')[0]
        key = hashlib.sha256(source.encode()).hexdigest()
        selected.setdefault(key, {'name': row['name'], 'source': source})

    with tempfile.TemporaryDirectory(prefix='monty-ty-comparison-') as tmp:
        project = Path(tmp)
        (project / 'src').mkdir()
        dependencies = '\n'.join(
            f'{crate} = {{ path = {json.dumps(str(root / "crates" / crate))} }}'
            for crate in ('monty-type-checking', 'monty-types', 'monty-alloc')
        )
        (project / 'Cargo.toml').write_text(
            '[package]\nname = "nesting-type-check"\nversion = "0.0.0"\nedition = "2024"\n'
            + '[dependencies]\n'
            + dependencies
            + '\n'
        )
        (project / 'src/main.rs').write_text(Path(__file__).with_name('type_check.rs').read_text())
        (project / 'Cargo.lock').write_bytes((root / 'Cargo.lock').read_bytes())
        target = args.target_dir.resolve() if args.target_dir else directory / 'ty-target'
        subprocess.run(
            [
                'cargo',
                'build',
                *([] if args.online else ['--offline']),
                '--release',
                '--manifest-path',
                str(project / 'Cargo.toml'),
                '--target-dir',
                str(target),
            ],
            check=True,
        )
        binary = target / 'release/nesting-type-check'
        with (directory / 'type-results.jsonl').open('w') as output:
            for key, row in selected.items():
                if key in cached:
                    output.write(json.dumps({'name': row['name'], **cached[key], 'reused': True}) + '\n')
                    output.flush()
                    continue
                try:
                    run = subprocess.run([str(binary)], input=row['source'], text=True, capture_output=True, timeout=10)
                    outcome = (
                        {'status': run.stdout.strip()}
                        if run.returncode == 0
                        else {'status': 'process-error', 'exit': run.returncode, 'stderr': run.stderr[-2000:]}
                    )
                except subprocess.TimeoutExpired:
                    outcome = {'status': 'timeout'}
                output.write(json.dumps({'name': row['name'], **outcome}) + '\n')
                output.flush()
        print(f'Type-checked {len(selected)} distinct Tokspam-accepted sources.')


if __name__ == '__main__':
    main()
