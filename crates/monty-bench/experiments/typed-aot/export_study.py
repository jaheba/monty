"""Persist text evidence and source patches without copying built executables."""
import hashlib
import json
import shutil
import tarfile
from pathlib import Path

source = Path(__file__).resolve().parent
destination = Path('/Users/jaheba/p/monty/crates/monty-bench/experiments/typed-aot')
assert not destination.exists(), destination
destination.mkdir(parents=True)
suffixes = {'.md', '.json', '.csv', '.py', '.log', '.patch', '.txt', '.html', '.stderr', '.stdout'}
folders = {'workloads', 'fuzz-runs', 'initial-artifacts', 'compact-artifacts', 'combined-budget-artifacts',
           'final-combined-artifacts', 'final-match-artifacts', 'liveness-combined-artifacts',
           'liveness-match-artifacts', 'default-artifacts', 'unified-combined-artifacts',
           'unified-match-artifacts', 'guarded-combined-artifacts', 'guarded-match-artifacts',
           'guarded-confirmation', 'fallback-confirmation', 'unified-confirmation', 'full-confirmation',
           'full-match-confirmation', 'liveness-confirmation', 'shared-threaded-confirmation',
           'budget-confirmation', 'cold-varied-validation', 'warm-varied-validation',
           'guarded-cold-validation', 'guarded-warm-validation'}
for path in source.rglob('*'):
    relative = path.relative_to(source)
    if not path.is_file() or path.suffix not in suffixes or 'bin' in relative.parts:
        continue
    if relative.parts[0] == 'profiles':
        continue
    target = destination / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(path, target)
dependency = source.parents[4] / 'spam'
archive = destination / 'dependency-source.tar.gz'
with tarfile.open(archive, 'w:gz', compresslevel=3) as tar:
    for path in sorted(dependency.rglob('*')):
        relative = path.relative_to(dependency)
        if path.is_file() and '.git' not in relative.parts and 'target' not in relative.parts:
            tar.add(path, arcname=Path('spam') / relative, recursive=False)
files = {str(p.relative_to(destination)): hashlib.sha256(p.read_bytes()).hexdigest()
         for p in destination.rglob('*') if p.is_file()}
(destination / 'bundle-sha256.json').write_text(json.dumps(files, indent=2) + '\n')
print(json.dumps({'destination': str(destination), 'files': len(files),
                  'bytes': sum(p.stat().st_size for p in destination.rglob('*') if p.is_file()),
                  'executables': 'Retained only in the isolated temporary study; build from source patches.'}))
