"""Write tables of depth scores, decision differences, and exploratory timings."""

import json
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path


def summarize(directory):
    rows = [json.loads(line) for line in (directory / 'results.jsonl').read_text().splitlines()]
    metadata = json.loads((directory / 'metadata.json').read_text())
    errors = [row for row in rows if 'process_error' in row]
    measured = [row for row in rows if 'process_error' not in row]
    lines = [
        '# Monty / Tokspam comparison',
        '',
        f'Monty: `{metadata["monty_revision"]}`.',
        f'Tokspam: `{metadata["spam_revision"]}`.',
        f'Compiler: `{metadata["rustc"]}`.',
        '',
        f'{len(rows)} inputs; {len(errors)} process failures.',
        f'{sum(r["monty_type_ok"] is None for r in measured)} type-check comparisons skipped because the forced '
        'parser scan rejects input that the 4 KiB shortcut permits.',
        '',
        'Limits are 200 for parser and AST depth, and 8 for quoted annotations.',
        'The adjusted policy uses AST depth 199; it changes no other limit.',
        '',
        '## Decisions',
        '',
        '| Group | Inputs | Parser differences | Type differences | Adjusted type differences |',
        '| --- | ---: | ---: | ---: | ---: |',
    ]
    if metadata.get('spam_diff_sha256'):
        lines[5:5] = [f'Tokspam tracked patch SHA-256: `{metadata["spam_diff_sha256"]}`.']

    def different(row, kind, spam_key=None):
        old = row[f'monty_{kind}_ok']
        return old is not None and old != row[spam_key or f'spam_{kind}_ok']

    groups = defaultdict(list)
    for row in measured:
        name = row['name']
        group = (
            'Monty fixtures'
            if 'test_cases/' in name
            else 'Attack corpus'
            if name.startswith('attack:')
            else 'Generated'
        )
        if name.endswith('#forced'):
            group += ' (forced scan)'
        groups[group].append(row)
    for name, group in groups.items():
        counts = (
            sum(different(r, 'parser') for r in group),
            sum(different(r, 'type') for r in group),
            sum(different(r, 'type', 'spam_adjusted_type_ok') for r in group),
        )
        lines.append(f'| {name} | {len(group)} | {counts[0]} | {counts[1]} | {counts[2]} |')

    if errors:
        lines += ['', '## Process failures', '', 'Failures are excluded from the decision and timing tables.', '']
        for row in errors:
            lines += [
                f'### {row["name"]}',
                '',
                f'Exit: `{row["process_error"]}`.',
                '',
                '```text',
                row.get('stderr', 'timeout'),
                '```',
                '',
            ]

    lines += [
        '',
        '## Scores at ten repetitions',
        '',
        'Monty scores are the smallest limit at which its existing check passes.',
        'Tokspam scores come from unlimited `analyze`; stopped scans have partial scores.',
        '',
        'A null Monty score means either the score exceeds the probe cap of 512, or AST parsing was skipped.',
        '',
        '| Shape | Monty parser | Spam parser | Monty AST | Spam AST |',
        '| --- | ---: | ---: | ---: | ---: |',
    ]
    for row in measured:
        if row['name'].endswith('/10/0/raw'):
            lines.append(
                f'| {row["name"].split("/")[0]} | {row["monty_parser_depth"]} | '
                f'{row["spam_parser_depth"]} | {row["monty_ast_depth"]} | {row["spam_ast_depth"]} |'
            )

    lines += [
        '',
        '## Decision differences by shape',
        '',
        'Each shape includes raw forced scans, raw scans with the 4 KiB shortcut, and padded sources above 4 KiB.',
        'Counts include syntactically invalid sources; a guard accepting input does not mean Python accepts it.',
        '',
        '| Shape | Parser differences | Type differences | First differing input |',
        '| --- | ---: | ---: | --- |',
    ]
    shapes = defaultdict(list)
    for row in measured:
        if different(row, 'parser') or different(row, 'type'):
            shapes[row['name'].split('/')[0] if row in groups['Generated'] else row['name']].append(row)
    for name, group in shapes.items():
        lines.append(
            f'| {name} | {sum(different(r, "parser") for r in group)} | '
            f'{sum(different(r, "type") for r in group)} | `{group[0]["name"]}` |'
        )

    lines += [
        '',
        '## Exploratory timings',
        '',
        'Release build; per-input averages over 3–30 calls, with reusable Tokspam checkers.',
        'These short runs are not a stable benchmark. Timings exclude input generation and score probing.',
        'Ratios below are the median of Monty time / Tokspam time, restricted to inputs both guards accept.',
        'Parser timings additionally exclude sources skipped by the length shortcut.',
        '',
        '| Group | Parser ratio | Type ratio |',
        '| --- | ---: | ---: |',
    ]
    for name, group in groups.items():
        ratios = {}
        for kind in ('parser', 'type'):
            values = [
                r[f'monty_{kind}_ns'] / r[f'spam_{kind}_ns']
                for r in group
                if r[f'monty_{kind}_ok']
                and r[f'spam_{kind}_ok']
                and r[f'spam_{kind}_ns']
                and (kind != 'parser' or r['bytes'] > r['threshold'])
            ]
            ratios[kind] = f'{statistics.median(values):.2f}×' if values else 'n/a'
        lines.append(f'| {name} | {ratios["parser"]} | {ratios["type"]} |')
    lines += ['', 'Raw inputs, results, and revision metadata are in the output directory.', '']
    type_results = directory / 'type-results.jsonl'
    if type_results.exists():
        checked = [json.loads(line) for line in type_results.read_text().splitlines()]
        counts = Counter(row['status'] for row in checked)
        reused = sum(row.get('reused', False) for row in checked)
        lines += [
            '## Actual type checker',
            '',
            'The probe calls `TypeChecker::run` directly in a separate process for each accepted source.',
            'It uses a 10-second timeout and a 512 MiB allocator ceiling. Diagnostics are collected but not rendered.',
            'Padding and duplicate sources are removed. Fixture files are excluded from this phase.',
            f'{len(checked)} accepted sources: {reused} reused identical-source results and {len(checked) - reused} new runs.',
            '',
            '| Outcome | Count |',
            '| --- | ---: |',
        ]
        for status, count in counts.items():
            lines.append(f'| {status} | {count} |')
        lines += ['', '| Resource failure | Outcome | Existing guard accepts |', '| --- | --- | --- |']
        by_name = {row['name']: row for row in measured}
        failed = [row for row in checked if row['status'] not in ('clean', 'diagnostics')]
        for row in failed:
            old = by_name[row['name']]['monty_type_ok']
            lines.append(f'| `{row["name"]}` | {row["status"]} | {old} |')
        old_accepts = sum(by_name[row['name']]['monty_type_ok'] is True for row in failed)
        memory_aborts = sum('memory limit' in row.get('stderr', '') for row in failed)
        lines += [
            '',
            f'{memory_aborts} process errors report allocator-limit aborts.',
            f'{old_accepts} of {len(failed)} resource failures also pass the existing guard.',
            'Flat source width and type-inference work can exhaust resources without a deep expression tree.',
            '',
        ]
    return '\n'.join(lines)


if __name__ == '__main__':
    print(summarize(Path(sys.argv[1])))
