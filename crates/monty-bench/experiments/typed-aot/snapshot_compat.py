"""Check suspended dumps across typed backends and the pre-AOT interpreter."""
import argparse
import hashlib
import json
import time
from pathlib import Path

from flying_circus.protocol import monty_pb2 as pb
from flying_circus.workers import MontyWorker


def event(worker, request, expected):
    worker.send(request)
    reply = worker.receive(time.perf_counter() + worker.timeout)
    assert reply.WhichOneof('kind') == expected, reply
    return getattr(reply, expected)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--typed', type=Path, required=True)
    parser.add_argument('--interpreter', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    modes = {'copied': (args.typed, ['--aot', '--aot-copied']), 'shared': (args.typed, ['--aot', '--aot-shared']),
             'interpreter': (args.interpreter, ['--no-native'])}
    code = ('def total(data: list[float]) -> float:\n'
            '    result = 0.0\n    for value in data:\n        result += value * 2.0\n    return result\n'
            'before = total([1.0,2.0])\nvalue = ext(before)\ntotal([value,1.0])')
    records = []
    for producer in ['copied', 'shared']:
        binary, arguments = modes[producer]
        with MontyWorker(binary, 30.0, arguments) as worker:
            worker.ready()
            event(worker, pb.ParentRequest(configure=pb.Configure(
                script_name='snapshot.py', protocol_version=worker.protocol_version,
                monty_version='typed-aot-study', type_check=True,
                type_check_stubs='def ext(value: float) -> float: ...',
            )), 'ok')
            call = event(worker, pb.ParentRequest(feed=pb.Feed(code=code)), 'function_call')
            assert call.values.nodes[call.args[0]].float == 6.0
            dump = event(worker, pb.ParentRequest(dump=pb.Dump()), 'dump_result').state
        for consumer, (binary, arguments) in modes.items():
            with MontyWorker(binary, 30.0, arguments) as worker:
                worker.ready()
                call = event(worker, pb.ParentRequest(load=pb.Load(state=dump)), 'function_call')
                assert call.function_name == 'ext'
                value = pb.MontyNode()
                setattr(value, 'float', 10.0)
                result = event(worker, pb.ParentRequest(resume_call=pb.ResumeCall(
                    call_id=call.call_id, result=pb.ExtFunctionResult(return_value=0),
                    values=pb.Arena(node_count=1, nodes=[value]),
                )), 'complete')
                assert result.values.nodes[result.value].float == 22.0, result
            records.append({'producer': producer, 'consumer': consumer, 'bytes': len(dump), 'result': 22.0})
    report = {'typed_sha256': hashlib.sha256(args.typed.read_bytes()).hexdigest(),
              'interpreter_sha256': hashlib.sha256(args.interpreter.read_bytes()).hexdigest(), 'checks': records}
    args.output.write_text(json.dumps(report, indent=2) + '\n')
    print(json.dumps(report))


if __name__ == '__main__':
    main()
