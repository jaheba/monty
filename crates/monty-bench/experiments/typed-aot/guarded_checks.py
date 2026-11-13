"""Validate independent dispatch features, then freeze the fuel-accounting experiment."""
import os
import shutil
import subprocess
from pathlib import Path

main = Path(__file__).resolve().parents[4]
out = Path(__file__).resolve().parent
env = os.environ | {'CARGO_TARGET_DIR': '/Users/jaheba/p/monty/target',
                    'PYO3_PYTHON': '/Users/jaheba/p/monty/.venv/bin/python'}
checks = [
    ('guarded-clippy.log', ['cargo', 'clippy', '-p', 'monty', '-p', 'monty-runtime', '--all-targets', '--features',
                            'monty-runtime/experimental-aot,monty-runtime/experimental-threaded,monty/memory-model-checks', '--', '-D', 'warnings']),
    ('guarded-full-tests.log', ['cargo', 'test', '-p', 'monty', '--features', 'experimental-aot,experimental-threaded,memory-model-checks,test-hooks']),
    ('guarded-match-tests.log', ['cargo', 'test', '-p', 'monty', '--features', 'experimental-aot,memory-model-checks', '--test', 'typed_aot']),
    ('guarded-worker-tests.log', ['cargo', 'test', '-p', 'monty-runtime', '--features', 'experimental-aot,experimental-threaded,monty/memory-model-checks', '--test', 'subprocess', 'typed_aot']),
    ('guarded-fixtures.log', ['cargo', 'run', '-p', 'monty-datatest', '--features', 'experimental-aot,monty/experimental-threaded,memory-model-checks', '--', 'run_test_cases_monty']),
    ('guarded-combined-build.log', ['cargo', 'build', '--offline', '--locked', '--release', '-p', 'monty-runtime', '--features', 'experimental-aot,experimental-threaded']),
    ('freeze-guarded-combined.log', ['python3', str(out / 'freeze.py'), '--out', str(out / 'guarded-combined-artifacts'), '--binary', '/Users/jaheba/p/monty/target/release/monty', '--dispatch', 'threaded feature; shared AOT; local liveness; checkpoint diagnostic accounting', '--features', 'experimental-aot', 'experimental-threaded']),
    ('guarded-stress-build.log', ['cargo', 'build', '--offline', '--locked', '--release', '-p', 'monty', '--features', 'experimental-aot,experimental-threaded', '--example', 'aot_differential']),
    ('guarded-match-build.log', ['cargo', 'build', '--offline', '--locked', '--release', '-p', 'monty-runtime', '--features', 'experimental-aot']),
    ('freeze-guarded-match.log', ['python3', str(out / 'freeze.py'), '--out', str(out / 'guarded-match-artifacts'), '--binary', '/Users/jaheba/p/monty/target/release/monty', '--dispatch', 'match; shared AOT; local liveness; checkpoint diagnostic accounting']),
    ('guarded-examples-build.log', ['cargo', 'build', '--offline', '--locked', '--release', '-p', 'monty', '--features', 'experimental-aot', '--example', 'typed_aot', '--example', 'aot_differential']),
]
for name, command in checks:
    with (out / name).open('w') as log:
        result = subprocess.run(command, cwd=main, env=env, stdout=log, stderr=subprocess.STDOUT)
    print(name, result.returncode, flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
    if name == 'guarded-stress-build.log':
        shutil.copy2('/Users/jaheba/p/monty/target/release/examples/aot_differential',
                     out / 'guarded-combined-artifacts/aot_differential')
example_out = out / 'guarded-example-artifacts'
example_out.mkdir()
for name in ['typed_aot', 'aot_differential']:
    shutil.copy2(Path('/Users/jaheba/p/monty/target/release/examples') / name, example_out / name)
