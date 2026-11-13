"""Measure fresh typed submissions through Flying Circus's Monty protocol client."""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import statistics
import subprocess
import time
from pathlib import Path

from flying_circus.protocol import monty_pb2 as pb
from flying_circus.workers import MontyWorker


class ValidatingWorker(MontyWorker):
    def __init__(self, binary: Path, arguments: list[str], validate: bool):
        self.validate = validate
        super().__init__(binary, 60.0, arguments)

    def configure(self, filename: str):
        self.request(pb.ParentRequest(configure=pb.Configure(
            script_name=filename, protocol_version=self.protocol_version,
            monty_version='typed-aot-study', type_check=self.validate,
        )), 'ok')


def fingerprint(path: Path):
    return {'path': str(path.resolve()), 'sha256': hashlib.sha256(path.read_bytes()).hexdigest()}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--samples', type=int, default=15)
    parser.add_argument('workloads', type=Path, nargs='+')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    records = []
    summaries = []
    modes = [(validate, aot) for validate in [False, True] for aot in [False, True]]
    for path in args.workloads:
        code = path.read_text()
        expected = subprocess.check_output(['/opt/homebrew/bin/python3', str(path)], text=True)
        workers = {mode: ValidatingWorker(args.binary, ['--aot' if mode[1] else '--no-native'], mode[0]) for mode in modes}
        try:
            for worker in workers.values():
                worker.ready()
            for sample in range(-3, args.samples):
                for mode in modes if sample % 2 == 0 else modes[::-1]:
                    worker = workers[mode]
                    start = time.perf_counter_ns()
                    worker.configure(path.name)
                    configured = time.perf_counter_ns()
                    stdout, stderr = worker.run(code, path.name)
                    ended = time.perf_counter_ns()
                    assert stdout == expected, (path, mode, stdout, expected)
                    assert not stderr, stderr
                    worker.reset()
                    if sample >= 0:
                        records.append({'workload': path.stem, 'validate': mode[0], 'aot': mode[1], 'sample': sample,
                                        'configure_ns': configured-start, 'submission_ns': ended-configured,
                                        'session_ns': ended-start})
            for mode in modes:
                selected = [r for r in records if r['workload'] == path.stem and (r['validate'], r['aot']) == mode]
                summary = {'workload': path.stem, 'validate': mode[0], 'aot': mode[1],
                           **{metric: statistics.median(r[metric] for r in selected) for metric in ['configure_ns','submission_ns','session_ns']}}
                summaries.append(summary)
                print(json.dumps(summary), flush=True)
        finally:
            for worker in workers.values():
                worker.close()
    metadata = {'binary': fingerprint(args.binary), 'harness': fingerprint(Path(__file__)),
                'worker_client': fingerprint(Path(__import__('flying_circus.workers', fromlist=['x']).__file__)),
                'workloads': [fingerprint(p) for p in args.workloads], 'samples': args.samples, 'warmups': 3,
                'host': platform.platform(), 'python': platform.python_version(),
                'boundary': 'Reused workers; fresh Configure and Feed each sample. Submission includes validation when requested; session includes Configure. Startup and Reset excluded.',
                'records': records, 'summaries': summaries}
    (args.output / 'results.json').write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    main()
