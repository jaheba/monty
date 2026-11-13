"""Run the paired prototype checks and freeze each release before the next build."""
import os
import subprocess
from pathlib import Path

main=Path(__file__).resolve().parents[4]
combined=main.parent/'threaded-aot'
out=Path(__file__).resolve().parent
assert (main/'Cargo.toml').is_file(),main
env=os.environ|{'CARGO_TARGET_DIR':'/Users/jaheba/p/monty/target','PYO3_PYTHON':'/Users/jaheba/p/monty/.venv/bin/python'}
steps=[
 ('liveness-clippy.log',combined,['cargo','clippy','-p','monty','-p','monty-runtime','--features','monty-runtime/experimental-aot,monty/memory-model-checks','--all-targets','--','-D','warnings']),
 ('liveness-full-tests.log',combined,['cargo','test','-p','monty','--features','experimental-aot,memory-model-checks,test-hooks']),
 ('liveness-fusion-tests.log',combined,['cargo','test','-p','monty','--features','experimental-aot,experimental-fusion,memory-model-checks,test-hooks','--test','dispatch_loop']),
 ('liveness-worker-tests.log',combined,['cargo','test','-p','monty-runtime','--features','experimental-aot,monty/memory-model-checks','--test','subprocess','typed_aot']),
 ('liveness-fixtures.log',combined,['cargo','run','--offline','--locked','-p','monty-datatest','--features','experimental-aot,memory-model-checks','--','run_test_cases_monty']),
 ('liveness-release-build.log',combined,['cargo','build','--offline','--locked','--release','-p','monty-runtime','--features','experimental-aot']),
 ('freeze-liveness-combined.log',main,['python3',str(out/'freeze.py'),'--root',str(combined),'--out',str(out/'liveness-combined-artifacts'),'--binary','/Users/jaheba/p/monty/target/release/monty','--dispatch','threaded; compact metadata; separate source fuel and handler depth; shared default']),
 ('liveness-combined-stress-build.log',combined,['cargo','build','--offline','--locked','--release','-p','monty','--features','experimental-aot','--example','aot_differential']),
 ('freeze-combined-stress.log',main,['python3','-c',"import shutil;shutil.copy2('/Users/jaheba/p/monty/target/release/examples/aot_differential','crates/monty-bench/experiments/typed-aot/liveness-combined-artifacts/aot_differential')"]),
 ('liveness-match-release-build.log',main,['cargo','build','--offline','--locked','--release','-p','monty-runtime','--features','experimental-aot']),
 ('freeze-liveness-match.log',main,['python3',str(out/'freeze.py'),'--out',str(out/'liveness-match-artifacts'),'--binary','/Users/jaheba/p/monty/target/release/monty','--dispatch','match; compact metadata; shared default']),
 ('liveness-examples-build.log',main,['cargo','build','--offline','--locked','--release','-p','monty','--features','experimental-aot','--example','typed_aot','--example','aot_differential']),
 ('default-release-build.log',main,['cargo','build','--offline','--locked','--release','-p','monty-runtime']),
 ('freeze-default.log',main,['python3',str(out/'freeze.py'),'--out',str(out/'default-artifacts'),'--binary','/Users/jaheba/p/monty/target/release/monty','--dispatch','match; no experimental features','--features']),
 ('wasm-fallback-check.log',main,['cargo','check','--offline','--locked','--target','wasm32-wasip1','-p','monty','--features','experimental-aot']),
]
for name,cwd,command in steps:
 with(out/name).open('w')as log:
  result=subprocess.run(command,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
 print(name,result.returncode,flush=True)
 if result.returncode:raise SystemExit(result.returncode)
