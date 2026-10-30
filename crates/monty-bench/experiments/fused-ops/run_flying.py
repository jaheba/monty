"""Compare worker requests with compilation included."""

import argparse
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('baseline', type=Path)
parser.add_argument('candidate', type=Path)
parser.add_argument('--output', required=True, type=Path)
args = parser.parse_args()
command = [
    'flying-circus',
    '--diff',
    '--threshold',
    '1',
    '--samples',
    '20',
    '--warmups',
    '3',
    '--memory-samples',
    '3',
    '--startup-samples',
    '5',
    '--format',
    'html',
    '--format',
    'markdown',
    '--format',
    'json',
    '--output',
    str(args.output),
    '-r',
    f'baseline={args.baseline.resolve()}',
    '-r',
    f'candidate={args.candidate.resolve()}',
]
for path in sorted((Path(__file__).parent / 'workloads').glob('*.py')):
    command.extend(['--benchmark', str(path)])
subprocess.run(command, check=True)
