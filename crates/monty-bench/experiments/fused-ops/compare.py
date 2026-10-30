"""Alternate parse-once runners and retain every timing sample."""

import argparse
import csv
import io
import statistics
import subprocess
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('baseline', type=Path)
parser.add_argument('candidate', type=Path)
parser.add_argument('--output', required=True, type=Path)
parser.add_argument('--rounds', type=int, default=3)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
workloads = sorted((Path(__file__).parent / 'workloads').glob('*.py'))
rows = {'baseline': [], 'candidate': []}
for round_index in range(args.rounds):
    order = ['baseline', 'candidate'] if round_index % 2 == 0 else ['candidate', 'baseline']
    for label in order:
        result = subprocess.run(
            [str(getattr(args, label).resolve()), *map(str, workloads)],
            check=True,
            capture_output=True,
            text=True,
        )
        for row in csv.DictReader(io.StringIO(result.stdout)):
            rows[label].append({'round': round_index, **row})
        print(f'Round {round_index + 1}: {label} complete', flush=True)
for label, measurements in rows.items():
    with (args.output / f'{label}.csv').open('w', newline='') as output:
        writer = csv.DictWriter(output, fieldnames=list(measurements[0]), lineterminator='\n')
        writer.writeheader()
        writer.writerows(measurements)
report = ['| Workload | Baseline ms | Fused ms | Change | Paired round changes |', '| --- | ---: | ---: | ---: | --- |']
for case in dict.fromkeys(row['case'] for row in rows['baseline']):
    groups = {label: [r for r in samples if r['case'] == case] for label, samples in rows.items()}
    medians = {label: statistics.median(int(r['ns']) for r in samples) for label, samples in groups.items()}
    changes = []
    for i in range(args.rounds):
        pair = [
            statistics.median(int(r['ns']) for r in groups[label] if r['round'] == i)
            for label in ['baseline', 'candidate']
        ]
        changes.append(pair[1] / pair[0] - 1)
    report.append(
        f'| {case} | {medians["baseline"] / 1e6:.3f} | {medians["candidate"] / 1e6:.3f} '
        f'| {medians["candidate"] / medians["baseline"] - 1:+.1%} '
        f'| {min(changes):+.1%} to {max(changes):+.1%} |'
    )
(args.output / 'results.md').write_text('\n'.join(report) + '\n')
print('\n'.join(report))
