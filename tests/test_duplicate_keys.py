"""Round 2 pins for the duplicate JSON key refusal."""

from pathlib import Path

import pytest

from catchledger import CatchledgerError
from catchledger.cli import _read_json, main


ROOT = Path(__file__).resolve().parents[1]


def _write(tmp_path, name, text):
    path = tmp_path / name
    path.write_text(text, encoding="utf-8")
    return path


def _args(tmp_path, ci_history, extra=()):
    return [
        "--ci-history",
        str(ci_history),
        "--git-log",
        str(ROOT / "examples" / "synthetic-git-log.txt"),
        *extra,
        "--output",
        str(tmp_path / "ledger.json"),
        "--report",
        str(tmp_path / "report.md"),
        "--packs-dir",
        str(tmp_path / "packs"),
    ]


def test_duplicate_keys_in_the_workflow_run_export_are_refused(tmp_path, capsys):
    export = _write(
        tmp_path,
        "runs.json",
        '{"repo": "attacker-label", "repo": "synthetic-ledger-repo",'
        ' "workflow_runs": []}',
    )
    result = main(_args(tmp_path, export))
    assert result == 2
    assert (
        "catchledger error: the workflow run export carries the key repo"
        " more than once"
        in capsys.readouterr().err
    )


def test_duplicate_keys_refuse_at_any_depth_and_name_the_first_repeat(tmp_path):
    inner_first = _write(
        tmp_path,
        "inner.json",
        '{"z": {"a": 1, "a": 2}, "b": 1, "b": 2}',
    )
    with pytest.raises(CatchledgerError) as caught:
        _read_json(inner_first, "workflow run export")
    assert (
        str(caught.value)
        == "the workflow run export carries the key a more than once"
    )

    two_repeats = _write(
        tmp_path, "two.json", '{"b": 1, "a": 2, "b": 3, "a": 4}'
    )
    with pytest.raises(CatchledgerError) as caught:
        _read_json(two_repeats, "workflow run export")
    assert (
        str(caught.value)
        == "the workflow run export carries the key b more than once"
    )

    in_list = _write(
        tmp_path, "list.json", '{"runs": [{"id": 1, "id": 2}]}'
    )
    with pytest.raises(CatchledgerError) as caught:
        _read_json(in_list, "workflow run export")
    assert (
        str(caught.value)
        == "the workflow run export carries the key id more than once"
    )


def test_duplicate_keys_in_the_canaryfield_result_and_a_written_pack_are_refused(
    tmp_path, capsys
):
    canary = _write(
        tmp_path,
        "canary.json",
        '{"gates": ["unit checks"], "gates": ["style checks"], "rows": []}',
    )
    result = main(
        _args(
            tmp_path,
            ROOT / "examples" / "synthetic-runs.json",
            extra=("--canaryfield-results", str(canary)),
        )
    )
    assert result == 2
    assert (
        "catchledger error: the canaryfield result carries the key gates"
        " more than once"
        in capsys.readouterr().err
    )

    pack = _write(
        tmp_path,
        "pack.json",
        '{"check": "unit checks", "check": "style checks"}',
    )
    with pytest.raises(CatchledgerError) as caught:
        _read_json(pack, "written evidence pack")
    assert (
        str(caught.value)
        == "the written evidence pack carries the key check more than once"
    )


def test_the_same_key_in_different_objects_is_accepted(tmp_path):
    path = _write(
        tmp_path, "ok.json", '{"runs": [{"id": 1}, {"id": 2}], "id": 3}'
    )
    document, _digest = _read_json(path, "workflow run export")
    assert document["id"] == 3
    assert [run["id"] for run in document["runs"]] == [1, 2]
