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
 ('budget-clippy.log',combined,['cargo','clippy','-p','monty','-p','monty-runtime','--features','monty-runtime/experimental-aot,monty/memory-model-checks','--all-targets','--','-D','warnings']),
 ('budget-full-tests.log',combined,['cargo','test','-p','monty','--features','experimental-aot,memory-model-checks,test-hooks']),
 ('budget-fusion-tests.log',combined,['cargo','test','-p','monty','--features','experimental-aot,experimental-fusion,memory-model-checks,test-hooks','--test','dispatch_loop']),
 ('budget-worker-tests.log',combined,['cargo','test','-p','monty-runtime','--features','experimental-aot,monty/memory-model-checks','--test','subprocess','typed_aot']),
 ('budget-release-build.log',combined,['cargo','build','--offline','--locked','--release','-p','monty-runtime','--features','experimental-aot']),
 ('freeze-budget.log',main,['python3',str(out/'freeze.py'),'--root',str(combined),'--out',str(out/'combined-budget-artifacts'),'--binary','/Users/jaheba/p/monty/target/release/monty','--dispatch','threaded; compact metadata; separate source fuel and handler depth; shared default']),
 ('compact-release-build.log',main,['cargo','build','--offline','--locked','--release','-p','monty-runtime','--features','experimental-aot']),
 ('freeze-compact.log',main,['python3',str(out/'freeze.py'),'--out',str(out/'compact-artifacts'),'--binary','/Users/jaheba/p/monty/target/release/monty','--dispatch','match; compact metadata; shared default']),
 ('compact-examples-build.log',main,['cargo','build','--offline','--locked','--release','-p','monty','--features','experimental-aot','--example','typed_aot','--example','aot_differential']),
]
for name,cwd,command in steps:
 with(out/name).open('w')as log:
  result=subprocess.run(command,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
 print(name,result.returncode,flush=True)
 if result.returncode:raise SystemExit(result.returncode)
