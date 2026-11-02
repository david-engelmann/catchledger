"""Round 1 review fix pins for catchledger slice 2."""

import copy
import json
from pathlib import Path

import pytest

from catchledger import (
    PackValidationError,
    build_evidence_packs,
    build_ledger,
    normalize_canary_document,
    parse_ci_document,
    parse_git_log,
    validate_evidence_pack,
)
from catchledger.models import CommitChange, CommitRecord, parse_timestamp
from synthetic_data import A, B, C, D, make_commit, parsed_runs, raw_run


ROOT = Path(__file__).resolve().parents[1]


def _check(ledger, name="unit checks"):
    return next(check for check in ledger["checks"] if check["name"] == name)


def _record(ledger, identifier="1"):
    return next(
        item
        for item in _check(ledger)["catch_record"]
        if item["block_run_id"] == identifier
    )


def test_descendant_success_without_a_fix_ends_the_catch_search():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("docs/guide.md", "M")]),
        make_commit(C, [B], "2026-01-01T02:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
        raw_run(3, "success", commit=C, started="2026-01-01T02:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["evidence"]["reason"] == "later_success_without_flagged_fix"
    assert record["evidence"]["validating_run_id"] is None
    assert record["evidence"]["fix_commits"] == []


def test_reason_prefers_the_descendant_success_over_a_later_flagged_change():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("docs/guide.md", "M")]),
        make_commit(C, [B], "2026-01-01T02:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["evidence"]["reason"] == "later_success_without_flagged_fix"


def test_fix_committed_after_the_validating_run_started_earns_no_catch():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T05:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T02:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["evidence"]["reason"] == "later_success_without_flagged_fix"


def test_blocked_commit_without_changed_paths_still_earns_a_false_block():
    commits = [make_commit(A, [], "2026-01-01T00:00:00Z", [])]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=A, started="2026-01-01T00:20:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "false_block"
    assert record["flagged_paths"] == []
    assert record["evidence"]["validating_run_id"] == "2"
    assert record["evidence"]["fix_commits"] == []


def test_blocked_merge_commit_earns_no_catch():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("docs/guide.md", "M")]),
        make_commit(C, [A, B], "2026-01-01T02:00:00Z", []),
        make_commit(D, [C], "2026-01-01T03:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=C, started="2026-01-01T02:10:00Z"),
        raw_run(2, "success", commit=D, started="2026-01-01T03:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["flagged_paths"] == []
    assert record["evidence"]["reason"] == "later_success_without_flagged_fix"


def test_fix_under_a_name_the_flagged_set_does_not_carry_is_missed():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/old.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("src/new.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["flagged_paths"] == ["src/old.py"]
    assert record["evidence"]["reason"] == "later_success_without_flagged_fix"


def test_rename_inside_the_interval_intersects_on_either_name():
    rename = CommitRecord(
        commit=B,
        parents=(A,),
        committed_at=parse_timestamp("2026-01-01T01:00:00Z", "committed_at"),
        subject="Synthetic rename",
        changes=(
            CommitChange(status="R100", paths=("src/old.py", "src/new.py")),
        ),
    )
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/old.py", "A")]),
        rename,
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "catch"
    assert record["evidence"]["fix_commits"] == [B]
    assert record["evidence"]["validating_run_id"] == "2"


def test_runtime_falls_back_to_the_creation_timestamp_without_a_start():
    record = raw_run(
        1, "success", commit=A, started="2026-01-01T00:10:00Z", duration=60
    )
    del record["run_started_at"]
    scorecard = _check(build_ledger(parsed_runs(record), []))["scorecard"]
    assert scorecard["runtime_seconds"] == 65


def test_negative_runtime_contributes_zero():
    runs = parsed_runs(raw_run(1, "failure", commit=A, duration=-30))
    scorecard = _check(build_ledger(runs, []))["scorecard"]
    assert scorecard["blocks"] == 1
    assert scorecard["runtime_seconds"] == 0
    assert scorecard["runtime_minutes"] == 0.0


def test_flake_rate_rounds_half_up_on_the_boundary():
    records = [
        raw_run(
            index + 1,
            "success",
            commit=f"{index:040x}",
            started="2026-01-01T00:10:00Z",
        )
        for index in range(128)
    ]
    records.append(
        raw_run(
            129,
            "failure",
            commit=f"{0:040x}",
            started="2026-01-01T00:20:00Z",
        )
    )
    scorecard = _check(build_ledger(parsed_runs(*records), []))["scorecard"]
    assert scorecard["judged_commits"] == 128
    assert scorecard["flaky_commits"] == 1
    assert scorecard["flake_rate"] == 0.007813


def test_later_outcome_without_validating_success_is_named():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("docs/guide.md", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "failure", commit=B, started="2026-01-01T01:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert (
        record["evidence"]["reason"] == "later_outcome_without_validating_success"
    )


def test_in_progress_record_with_a_failure_conclusion_is_skipped_on_status():
    record = raw_run(1, "failure", status="in_progress")
    assert record["conclusion"] == "failure"
    assert parse_ci_document([record]) == []


def test_git_log_export_reads_copy_lines_beside_renames():
    lines = [
        "commit " + "a" * 40,
        "parents ",
        "date 2026-01-01T00:00:00Z",
        "subject Synthetic first commit",
        "A\tsrc/code.py",
        "",
        "commit " + "b" * 40,
        "parents " + "a" * 40,
        "date 2026-01-01T01:00:00Z",
        "subject Synthetic second commit",
        "C100\tsrc/code.py\tsrc/copy.py",
    ]
    records = parse_git_log("\n".join(lines) + "\n")
    copied = records[1].changes[-1]
    assert copied.status == "C100"
    assert copied.paths == ("src/code.py", "src/copy.py")
    assert records[1].changed_paths == ("src/code.py", "src/copy.py")


def _window_ledger():
    runs = parsed_runs(
        raw_run(1, "success", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-03T00:10:00Z"),
    )
    return build_ledger(runs, [])


def test_ledger_and_packs_carry_the_history_window():
    ledger = _window_ledger()
    expected = {
        "first_run_started_at": "2026-01-01T00:10:00Z",
        "last_run_updated_at": "2026-01-03T00:11:00Z",
    }
    assert ledger["history_window"] == expected
    check = _check(ledger)
    assert check["scorecard"]["blocks"] == 0
    pack = build_evidence_packs(ledger)[check["evidence_pack_file"]]
    assert pack["history_window"] == expected
    assert validate_evidence_pack(pack, ledger) is True


def test_history_window_is_null_when_no_runs_stand():
    ledger = build_ledger([], [])
    assert ledger["history_window"] is None
    assert ledger["checks"] == []


def test_pack_validation_refuses_a_tampered_history_window():
    ledger = _window_ledger()
    check = _check(ledger)
    original = build_evidence_packs(ledger)[check["evidence_pack_file"]]

    replaced = copy.deepcopy(original)
    replaced["history_window"] = {
        "first_run_started_at": "2020-01-01T00:00:00Z",
        "last_run_updated_at": "2020-01-02T00:00:00Z",
    }
    with pytest.raises(PackValidationError):
        validate_evidence_pack(replaced, ledger)

    nulled = copy.deepcopy(original)
    nulled["history_window"] = None
    with pytest.raises(PackValidationError):
        validate_evidence_pack(nulled, ledger)


def test_mutating_a_built_pack_moves_neither_ledger_nor_validation():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
    )
    ledger = build_ledger(runs, commits)
    check = _check(ledger)
    pack = build_evidence_packs(ledger)[check["evidence_pack_file"]]
    pack["scorecard"]["fires"] = 999
    pack["pricing"]["cost_per_minute"] = "9.999"
    pack["catch_record"].append({"outcome": "catch"})
    assert check["scorecard"]["fires"] == 2
    assert ledger["pricing"]["cost_per_minute"] == "0.008"
    assert len(check["catch_record"]) == 1
    with pytest.raises(PackValidationError):
        validate_evidence_pack(pack, ledger)


def test_canaryfield_rows_carry_exactly_the_stated_shape():
    document = json.loads(
        (ROOT / "examples" / "synthetic-canaryfield-results.json").read_text(
            encoding="utf-8"
        )
    )
    normalized = normalize_canary_document(document, "unit checks")
    for row in normalized["results"]:
        assert set(row) == {
            "seed",
            "name",
            "defect_class",
            "result",
            "planted_files",
        }

    custom = {
        "gates": ["unit checks"],
        "rows": [
            {
                "seed": "seed-10",
                "name": "singular planted file",
                "defect_class": "synthetic class",
                "gates": {"unit checks": "caught"},
                "planted_file": "src/only.py",
            },
            {
                "seed": "seed-11",
                "name": "no planted file",
                "defect_class": "synthetic class",
                "gates": {"unit checks": "waved-through"},
            },
        ],
    }
    normalized = normalize_canary_document(custom, "unit checks")
    assert normalized["results"][0]["planted_files"] == ["src/only.py"]
    assert normalized["results"][1]["planted_files"] == []
