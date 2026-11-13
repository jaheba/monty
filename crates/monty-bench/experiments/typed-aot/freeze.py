"""Freeze an executable and the uncommitted source needed to reproduce it."""
import argparse
import hashlib
import json
import shutil
import subprocess
from pathlib import Path

p = argparse.ArgumentParser()
p.add_argument('--root', type=Path, default=Path.cwd())
p.add_argument('--out', type=Path, required=True)
p.add_argument('--binary', type=Path, required=True)
p.add_argument('--dispatch', required=True)
p.add_argument('--features', nargs='*', default=['experimental-aot'])
a = p.parse_args()
a.root = a.root.resolve()
a.out.mkdir(parents=True, exist_ok=False)
(a.out / 'bin').mkdir()
shutil.copy2(a.binary, a.out / 'bin/monty')
def git(*args):
    return subprocess.check_output(['git', *args], cwd=a.root)
tracked = git('diff', '--name-only').decode().splitlines()
new = git('ls-files', '--others', '--exclude-standard').decode().splitlines()
paths = [s for s in dict.fromkeys(tracked + new) if '/experiments/' not in s and (a.root/s).is_file()]
patch = git('diff', '--binary')
for name in new:
    if name in paths:
        result = subprocess.run(['git', 'diff', '--no-index', '--binary', '/dev/null', name], cwd=a.root, capture_output=True)
        assert result.returncode == 1, result.stderr
        patch += result.stdout
(a.out/'source.patch').write_bytes(patch)
(a.out/'source-sha256.json').write_text(json.dumps({s:hashlib.sha256((a.root/s).read_bytes()).hexdigest() for s in paths},indent=2)+'\n')
metadata={'sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'bytes':a.binary.stat().st_size,
          'features':a.features,'profile':'release: fat LTO, one codegen unit, strip=true',
          'dispatch':a.dispatch,'baseline_commit':git('rev-parse','HEAD').decode().strip(),
          'rustc':subprocess.check_output(['rustc','--version'],text=True).strip()}
(a.out/'binary.json').write_text(json.dumps(metadata,indent=2)+'\n')
print(json.dumps(metadata))
