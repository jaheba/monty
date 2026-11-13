"""Measure fresh typed submissions through Flying Circus's Monty protocol client."""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import shutil
import statistics
import subprocess
import time
from pathlib import Path

import flying_circus.workers as client
from flying_circus.protocol import monty_pb2 as pb
from flying_circus.workers import MontyWorker


class ValidatingWorker(MontyWorker):
    def __init__(self, binary: Path, backend: str, validate: bool):
        self.validate = validate
        arguments = ['--no-native'] if backend == 'interpreter' else ['--aot']
        if backend == 'shared':
            arguments.append('--aot-shared')
        elif backend == 'copied':
            arguments.append('--aot-copied')
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
    parser.add_argument('--worker', choices=['warm', 'cold'], default='warm')
    parser.add_argument('--varied-source', action='store_true')
    parser.add_argument('--shared', action='store_true')
    parser.add_argument('workloads', type=Path, nargs='+')
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    shutil.copy2(__file__, args.output / 'harness.py')
    shutil.copy2(client.__file__, args.output / 'workers.py')
    records = []
    summaries = []
    backends = ['interpreter', 'copied', *(['shared'] if args.shared else [])]
    modes = [(validate, backend) for validate in [False, True] for backend in backends]
    warmups = 3 if args.worker == 'warm' else 0
    for path in args.workloads:
        code = path.read_text()
        expected = subprocess.check_output(['/opt/homebrew/bin/python3', str(path)], text=True)
        workers = {}
        try:
            if args.worker == 'warm':
                workers = {mode: ValidatingWorker(args.binary, mode[1], mode[0]) for mode in modes}
                for worker in workers.values():
                    worker.ready()
            for sample in range(-warmups, args.samples):
                for mode in modes if sample % 2 == 0 else modes[::-1]:
                    worker = workers.get(mode)
                    if worker is None:
                        worker = ValidatingWorker(args.binary, mode[1], mode[0])
                        worker.ready()
                    try:
                        filename = f'{path.stem}_{sample}.py' if args.varied_source else path.name
                        source = code + f'\n# submission {sample}\n' if args.varied_source else code
                        start = time.perf_counter_ns()
                        worker.configure(filename)
                        configured = time.perf_counter_ns()
                        stdout, stderr = worker.run(source, filename)
                        ended = time.perf_counter_ns()
                        assert stdout == expected, (path, mode, stdout, expected)
                        assert not stderr, stderr
                        worker.reset()
                        if sample >= 0:
                            records.append({'workload': path.stem, 'validate': mode[0], 'backend': mode[1], 'sample': sample,
                                            'configure_ns': configured-start, 'submission_ns': ended-configured,
                                            'session_ns': ended-start})
                    finally:
                        if args.worker == 'cold':
                            worker.close()
            for mode in modes:
                selected = [r for r in records if r['workload'] == path.stem and (r['validate'], r['backend']) == mode]
                summary = {'workload': path.stem, 'validate': mode[0], 'backend': mode[1],
                           **{metric: statistics.median(r[metric] for r in selected) for metric in ['configure_ns','submission_ns','session_ns']}}
                summaries.append(summary)
                print(json.dumps(summary), flush=True)
        finally:
            for worker in workers.values():
                worker.close()
    metadata = {'binary': fingerprint(args.binary), 'harness': fingerprint(Path(__file__)),
                'worker_client': fingerprint(Path(client.__file__)),
                'workloads': [fingerprint(p) for p in args.workloads], 'samples': args.samples, 'warmups': warmups,
                'worker': args.worker, 'varied_source': args.varied_source,
                'host': platform.platform(), 'python': platform.python_version(),
                'boundary': 'Fresh Configure and Feed each sample. Submission includes validation when requested; session includes Configure. Startup, readiness negotiation and Reset excluded.',
                'records': records, 'summaries': summaries}
    (args.output / 'results.json').write_text(json.dumps(metadata, indent=2) + '\n')


if __name__ == '__main__':
    main()
