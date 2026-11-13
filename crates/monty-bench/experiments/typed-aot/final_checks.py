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
 ('final-default-check.log',main,['cargo','check','--offline','--locked','-p','monty','-p','monty-runtime']),
 ('final-clippy.log',combined,['cargo','clippy','-p','monty','-p','monty-runtime','--features','monty-runtime/experimental-aot,monty/memory-model-checks','--all-targets','--','-D','warnings']),
 ('final-full-tests.log',combined,['cargo','test','-p','monty','--features','experimental-aot,memory-model-checks,test-hooks']),
 ('final-fusion-tests.log',combined,['cargo','test','-p','monty','--features','experimental-aot,experimental-fusion,memory-model-checks,test-hooks','--test','dispatch_loop']),
 ('final-worker-tests.log',combined,['cargo','test','-p','monty-runtime','--features','experimental-aot,monty/memory-model-checks','--test','subprocess','typed_aot']),
 ('final-fixtures.log',combined,['cargo','run','--offline','--locked','-p','monty-datatest','--features','experimental-aot,memory-model-checks','--','run_test_cases_monty']),
 ('final-release-build.log',combined,['cargo','build','--offline','--locked','--release','-p','monty-runtime','--features','experimental-aot']),
 ('freeze-final-combined.log',main,['python3',str(out/'freeze.py'),'--root',str(combined),'--out',str(out/'final-combined-artifacts'),'--binary','/Users/jaheba/p/monty/target/release/monty','--dispatch','threaded; compact metadata; separate source fuel and handler depth; shared default']),
 ('final-combined-stress-build.log',combined,['cargo','build','--offline','--locked','--release','-p','monty','--features','experimental-aot','--example','aot_differential']),
 ('freeze-combined-stress.log',main,['python3','-c',"import shutil;shutil.copy2('/Users/jaheba/p/monty/target/release/examples/aot_differential','crates/monty-bench/experiments/typed-aot/final-combined-artifacts/aot_differential')"]),
 ('final-match-release-build.log',main,['cargo','build','--offline','--locked','--release','-p','monty-runtime','--features','experimental-aot']),
 ('freeze-final-match.log',main,['python3',str(out/'freeze.py'),'--out',str(out/'final-match-artifacts'),'--binary','/Users/jaheba/p/monty/target/release/monty','--dispatch','match; compact metadata; shared default']),
 ('final-examples-build.log',main,['cargo','build','--offline','--locked','--release','-p','monty','--features','experimental-aot','--example','typed_aot','--example','aot_differential']),
]
for name,cwd,command in steps:
 with(out/name).open('w')as log:
  result=subprocess.run(command,cwd=cwd,env=env,stdout=log,stderr=subprocess.STDOUT)
 print(name,result.returncode,flush=True)
 if result.returncode:raise SystemExit(result.returncode)
