"""Run serial comparisons only after builds and stress have stopped."""
import subprocess
from pathlib import Path

out = Path(__file__).resolve().parent


def binary(folder):
    return str(out / folder / 'bin/monty')


common = ['flying-circus', '--diff', '--samples', '15', '--startup-samples', '5',
          '--warmups', '3', '-x1', '-v', '--threshold', '3',
          '--format', 'html', '--format', 'markdown', '--format', 'json']
selected = [*common, '--memory-samples', '3',
            '-r', 'default=' + binary('default-artifacts'),
            '-r', 'threaded=' + binary('liveness-combined-artifacts'),
            '-r', 'before=' + binary('final-combined-artifacts'),
            '-r', 'live=' + binary('liveness-combined-artifacts'),
            '-r', 'match=' + binary('liveness-match-artifacts'),
            '-r', 'CPython=/opt/homebrew/bin/python3',
            '--runtime-arg', 'threaded=--no-native',
            '--runtime-arg', 'before=--aot', '--runtime-arg', 'live=--aot',
            '--runtime-arg', 'match=--aot', '--output', str(out / 'liveness-confirmation')]
for path in sorted((out / 'workloads').glob('*.py')):
    selected.extend(['--benchmark', str(path)])
full = [*common, '--memory-samples', '0',
        '-r', 'default=' + binary('default-artifacts'),
        '-r', 'threaded=' + binary('liveness-combined-artifacts'),
        '-r', 'typed=' + binary('liveness-combined-artifacts'),
        '--runtime-arg', 'threaded=--no-native', '--runtime-arg', 'typed=--aot',
        '--output', str(out / 'full-confirmation')]
for name, command in [('liveness-confirmation', selected), ('full-confirmation', full)]:
    with (out / f'{name}.log').open('w') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    print(name, result.returncode, flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
with (out / 'liveness-timings.csv').open('w') as log:
    result = subprocess.run([str(out / 'liveness-example-artifacts/typed_aot'), '15'], stdout=log)
print('liveness-timings', result.returncode, flush=True)
raise SystemExit(result.returncode)
