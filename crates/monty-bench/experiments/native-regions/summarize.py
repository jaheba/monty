"""Summarize paired samples and retain per-case native coverage."""

import csv
import statistics
from collections import defaultdict
from pathlib import Path


def main():
    root = Path(__file__).parent
    with (root / 'timings.csv').open() as stream:
        rows = list(csv.DictReader(stream))

    samples = defaultdict(list)
    for row in rows:
        samples[row['case'], row['boundary'], row['mode']].append(row)

    names = list(dict.fromkeys(row['case'] for row in rows))
    header = [
        'case',
        'boundary',
        'interpreter_ns',
        'native_ns',
        'speedup',
        'native_instructions_per_entry',
        'compilation_ns_per_artifact',
        'dictionary_reads',
        'range_steps',
    ]
    summary = []
    for name in names:
        for boundary in ('fresh', 'retained'):
            baseline = samples[name, boundary, 'interpreter']
            native = samples[name, boundary, 'native']
            assert len(baseline) == len(native) == 9
            assert all(int(row['native_instructions']) == 0 for row in baseline)
            assert all(int(row['native_instructions']) > 0 for row in native)
            base_ns = statistics.median(int(row['ns']) for row in baseline)
            native_ns = statistics.median(int(row['ns']) for row in native)
            compilations = sum(int(row['compilations']) for row in native)
            instructions = sum(int(row['native_instructions']) for row in native)
            entries = sum(int(row['entries']) for row in native)
            compilation_ns = sum(int(row['compilation_ns']) for row in native)
            summary.append(
                [
                    name,
                    boundary,
                    base_ns,
                    native_ns,
                    f'{base_ns / native_ns:.3f}',
                    f'{instructions / entries:.1f}',
                    round(compilation_ns / compilations) if compilations else 0,
                    sum(int(row['dictionary_reads']) for row in native),
                    sum(int(row['range_steps']) for row in native),
                ]
            )

    with (root / 'summary.csv').open('w') as stream:
        writer = csv.writer(stream)
        writer.writerow(header)
        writer.writerows(summary)

    print('| Case | Fresh interpreter / native, µs | Fresh speedup | Retained speedup |')
    print('| --- | ---: | ---: | ---: |')
    for name in names:
        fresh, retained = [row for row in summary if row[0] == name]
        print(f'| {name} | {fresh[2] / 1000:.2f} / {fresh[3] / 1000:.2f} | {fresh[4]}× | {retained[4]}× |')


if __name__ == '__main__':
    main()
