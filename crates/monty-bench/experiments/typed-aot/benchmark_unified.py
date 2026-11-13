"""Measure the unified source, then check match-only AOT on the full corpus."""
import subprocess
from pathlib import Path

out = Path(__file__).resolve().parent


def binary(folder):
    return str(out / folder / 'bin/monty')


common = ['flying-circus', '--diff', '--samples', '15', '--startup-samples', '5',
          '--warmups', '3', '-x1', '-v', '--threshold', '3',
          '--format', 'html', '--format', 'markdown', '--format', 'json']
selected = [*common, '--memory-samples', '3',
            '-r', 'before=' + binary('liveness-combined-artifacts'),
            '-r', 'combined=' + binary('unified-combined-artifacts'),
            '-r', 'match_before=' + binary('liveness-match-artifacts'),
            '-r', 'match=' + binary('unified-match-artifacts'),
            '-r', 'default=' + binary('default-artifacts'),
            '-r', 'CPython=/opt/homebrew/bin/python3',
            '--runtime-arg', 'before=--aot', '--runtime-arg', 'combined=--aot',
            '--runtime-arg', 'match_before=--aot', '--runtime-arg', 'match=--aot',
            '--output', str(out / 'unified-confirmation')]
for path in sorted((out / 'workloads').glob('*.py')):
    selected.extend(['--benchmark', str(path)])
full = [*common, '--memory-samples', '0',
        '-r', 'default=' + binary('default-artifacts'),
        '-r', 'typed_match=' + binary('unified-match-artifacts'),
        '--runtime-arg', 'typed_match=--aot', '--output', str(out / 'full-match-confirmation')]
for name, command in [('unified-confirmation', selected), ('full-match-confirmation', full)]:
    with (out / f'{name}.log').open('w') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    print(name, result.returncode, flush=True)
    # The full suite reports known baseline failures with exit 1. Preserve the
    # exit code and inspect compatibility changes in its report separately.
    allowed = {0, 1} if name == 'full-match-confirmation' else {0}
    if result.returncode not in allowed:
        raise SystemExit(result.returncode)
with (out / 'unified-timings.csv').open('w') as log:
    result = subprocess.run([str(out / 'unified-example-artifacts/typed_aot'), '15'], stdout=log)
print('unified-timings', result.returncode, flush=True)
raise SystemExit(result.returncode)
