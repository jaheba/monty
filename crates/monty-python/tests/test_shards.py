from pathlib import Path

import pytest

from pydantic_monty import FunctionSnapshot, Monty, MontyComplete, MontyRuntimeError, MountDir


def test_dynamic_evaluation_is_optional(pool: Monty):
    with pool.checkout(shards=[]) as session:
        for code in ['eval("42")', 'exec(b"x = 42")', 'alias = eval', 'def lookup(): return exec\nlookup()']:
            with pytest.raises(MontyRuntimeError) as caught:
                session.feed_run(code)
            assert isinstance(caught.value.exception(), NameError)

        assert session.feed_run('def eval(x): return x + 1\neval(41)') == 42

    with pool.checkout(shards=['dynamic_evaluation']) as session:
        assert session.feed_run('alias = eval\nexec("n = 41")\nalias("n + 1")') == 42

    with pool.checkout() as session:
        assert session.feed_run('eval("42")') == 42


def test_shard_controls_string_annotations(pool: Monty):
    with pool.checkout(shards=[]) as session:
        session.feed_run('from typing import get_type_hints, ForwardRef\nfrom annotationlib import get_annotations')
        assert session.feed_run('def f(x: int): pass\nget_type_hints(f)["x"] is int') is True
        assert session.feed_run('def f(x: "int"): pass\nget_annotations(f)["x"]') == 'int'
        for code in ['get_type_hints(f)', 'ForwardRef("int").evaluate()', 'get_annotations(f, eval_str=True)']:
            with pytest.raises(MontyRuntimeError) as caught:
                session.feed_run(code)
            assert isinstance(caught.value.exception(), PermissionError)


def test_imported_module_uses_session_shards(pool: Monty, tmp_path: Path):
    (tmp_path / 'helper.py').write_text('answer = 42\ndef evaluate(text): return eval(text)\n')
    with pool.checkout(shards=[]) as session, MountDir(host_path=str(tmp_path), virtual_path='/src') as mount:
        assert session.feed_run('import helper\nhelper.answer', mount=mount) == 42
        with pytest.raises(MontyRuntimeError) as caught:
            session.feed_run('helper.evaluate("42")')
        assert isinstance(caught.value.exception(), NameError)


@pytest.mark.parametrize('suspended', [False, True])
def test_snapshot_requires_same_shards(pool: Monty, suspended: bool):
    with pool.checkout(shards=[]) as source:
        source.feed_run('def f(): return eval("42")')
        if suspended:
            call = source.feed_start('host()')
            assert isinstance(call, FunctionSnapshot)
            state = call.dump()
        else:
            state = source.dump()

    with pool.checkout(shards=[]) as destination:
        if suspended:
            loaded = destination.load_snapshot(state)
            assert isinstance(loaded, FunctionSnapshot)
            assert isinstance(loaded.resume({'return_value': None}), MontyComplete)
        else:
            destination.load_session(state)
        with pytest.raises(MontyRuntimeError) as caught:
            destination.feed_run('f()')
        assert isinstance(caught.value.exception(), NameError)

    with pool.checkout() as incompatible, pytest.raises(MontyRuntimeError, match='snapshot shards differ'):
        if suspended:
            incompatible.load_snapshot(state)
        else:
            incompatible.load_session(state)


def test_unknown_shard(pool: Monty):
    with pytest.raises(ValueError, match='unknown Monty shard'):
        pool.checkout(shards=['typo'])  # type: ignore[list-item]


def test_core_session_refuses_a_snapshot_with_an_eval_alias(pool: Monty):
    with pool.checkout() as source:
        source.feed_run('evaluate = eval')
        state = source.dump()

    with pool.checkout(shards=[]) as destination, pytest.raises(MontyRuntimeError, match='snapshot shards differ'):
        destination.load_session(state)
