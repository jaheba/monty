"""Compare final cache/entry guards and confirm previously regressing fallback workloads."""
import subprocess
from pathlib import Path

out = Path(__file__).resolve().parent
corpus = Path('/Users/jaheba/p/flying-circus/src/flying_circus/workloads')


def binary(folder):
    return str(out / folder / 'bin/monty')


common = ['flying-circus', '--diff', '--samples', '15', '--startup-samples', '5',
          '--warmups', '3', '-x1', '-v', '--threshold', '3',
          '--format', 'html', '--format', 'markdown', '--format', 'json']
selected = [*common, '--memory-samples', '0',
            '-r', 'default=' + binary('default-artifacts'),
            '-r', 'before=' + binary('unified-match-artifacts'),
            '-r', 'match=' + binary('guarded-match-artifacts'),
            '-r', 'combined=' + binary('guarded-combined-artifacts'),
            '-r', 'CPython=/opt/homebrew/bin/python3',
            '--runtime-arg', 'before=--aot', '--runtime-arg', 'match=--aot',
            '--runtime-arg', 'combined=--aot', '--output', str(out / 'guarded-confirmation')]
for path in sorted((out / 'workloads').glob('*.py')):
    selected.extend(['--benchmark', str(path)])
fallback = [*common, '--memory-samples', '0',
            '-r', 'default=' + binary('default-artifacts'),
            '-r', 'before=' + binary('unified-match-artifacts'),
            '-r', 'match=' + binary('guarded-match-artifacts'),
            '--runtime-arg', 'before=--aot', '--runtime-arg', 'match=--aot',
            '--output', str(out / 'fallback-confirmation')]
for name in ['portfolio_risk', 'text_csv_cleanup_medium', 'perf_richards', 'rustpython_pystone',
             'rustpython_micro_cmp', 'rustpython_micro_sorted_900', 'stdout_lines']:
    fallback.extend(['--benchmark', str(corpus / f'{name}.py')])
for name, command in [('guarded-confirmation', selected), ('fallback-confirmation', fallback)]:
    with (out / f'{name}.log').open('w') as log:
        result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
    print(name, result.returncode, flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
with (out / 'guarded-timings.csv').open('w') as log:
    result = subprocess.run([str(out / 'guarded-example-artifacts/typed_aot'), '15'], stdout=log)
print('guarded-timings', result.returncode, flush=True)
raise SystemExit(result.returncode)
