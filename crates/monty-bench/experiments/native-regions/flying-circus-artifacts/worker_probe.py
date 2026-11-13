"""Check that both comparison modes reach the protocol worker thread."""

import json
import re
import sys
from pathlib import Path

from flying_circus.workers import MontyWorker


def main():
    binary = Path(sys.argv[1]).resolve()
    records = {}
    code = 'total = 0\nfor i in range(1000):\n    total += i * 2\nprint(total)'
    for label, flags in (('baseline', ['--no-native']), ('native', [])):
        worker = MontyWorker(binary, 30, arguments=[*flags, '--native-stats'])
        try:
            worker.ready()
            worker.configure('probe.py')
            stdout, stderr = worker.run(code, 'probe.py')
            assert stdout == '999000\n' and not stderr, (stdout, stderr)
            worker.reset()
            worker.outgoing.put(None)
            worker.writer.join(timeout=5)
            worker.process.stdin.close()
            assert worker.process.wait(timeout=10) == 0
            worker.stderr.seek(0)
            diagnostics = worker.stderr.read().decode()
            stats = {key: int(value) for key, value in re.findall(r'(\w+): (\d+)', diagnostics)}
            assert (stats['instructions'] == 0) == (label == 'baseline'), diagnostics
            if label == 'native':
                assert stats['range_steps'] >= 900, diagnostics
            records[label] = {'arguments': flags, 'stdout': stdout, 'stats': stats}
        finally:
            worker.close()
    print(json.dumps(records, indent=2))


if __name__ == '__main__':
    main()
