"""Add annotations to frozen Flying Circus sources without changing their executable AST."""

import ast
import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT.parent / 'native-regions' / 'flying-circus-artifacts' / 'workloads'
ANNOTATIONS = {
    'perf_unpack_sequence': {
        'def do_unpacking(loops, to_unpack):': 'def do_unpacking(loops: int, to_unpack: tuple[int, ...] | list[int]) -> tuple[int, ...]:',
        'def bench_tuple_unpacking(loops):': 'def bench_tuple_unpacking(loops: int) -> tuple[int, ...]:',
        'def bench_list_unpacking(loops):': 'def bench_list_unpacking(loops: int) -> tuple[int, ...]:',
        'def bench_all(loops):': 'def bench_all(loops: int) -> tuple[int, ...]:',
    },
    'game_spectral_norm': {
        'def eval_A(i, j):': 'def eval_A(i: int, j: int) -> float:',
        'def part_A_times_u(i_u):': 'def part_A_times_u(i_u: tuple[int, list[float]]) -> float:',
        'def part_At_times_u(i_u):': 'def part_At_times_u(i_u: tuple[int, list[float]]) -> float:',
        'def bench_spectral_norm(loops):': 'def bench_spectral_norm(loops: int) -> float:',
    },
    'capacity_planning': {
        'def allocate(projects, budget):': 'def allocate(projects: list[tuple[int, int]], budget: int) -> tuple[int, list[int]]:',
        'def plan():': 'def plan() -> None:',
    },
    'perf_float': {
        '    def __init__(self, i):': '    def __init__(self, i: float) -> None:',
        '    def normalize(self):': '    def normalize(self) -> None:',
        'def benchmark(n):': 'def benchmark(n: int) -> Point:',
        'def maximize(points):': 'def maximize(points: list[Point]) -> Point:',
    },
    'ticket_search': {'def search():': 'def search() -> None:'},
}


class EraseAnnotations(ast.NodeTransformer):
    def visit_arg(self, node):
        node.annotation = None
        return node

    def visit_FunctionDef(self, node):
        node.returns = None
        return self.generic_visit(node)


def executable_tree(source):
    tree = EraseAnnotations().visit(copy.deepcopy(ast.parse(source)))
    return ast.dump(tree, include_attributes=False)


def main():
    manifest = []
    for name, replacements in ANNOTATIONS.items():
        original = (SOURCE / f'{name}.py').read_text()
        candidate = original
        for old, new in replacements.items():
            assert candidate.count(old) == 1, (name, old)
            candidate = candidate.replace(old, new)
        assert executable_tree(original) == executable_tree(candidate), name
        destination = ROOT / 'workloads' / f'{name}.py'
        destination.write_text(candidate)
        manifest.append({
            'workload': name,
            'original_sha256': hashlib.sha256(original.encode()).hexdigest(),
            'typed_sha256': hashlib.sha256(candidate.encode()).hexdigest(),
            'only_annotations_changed': True,
            'replacements': replacements,
        })
    (ROOT / 'workloads-manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')


if __name__ == '__main__':
    main()
