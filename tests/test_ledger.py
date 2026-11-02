import copy
import json
from pathlib import Path

import pytest

from catchledger import (
    CatchledgerError,
    PackValidationError,
    build_evidence_packs,
    build_ledger,
    normalize_canary_document,
    validate_evidence_pack,
)
from synthetic_data import A, B, C, D, MISSING, make_commit, parsed_runs, raw_run


ROOT = Path(__file__).resolve().parents[1]


def _check(ledger, name="unit checks"):
    return next(check for check in ledger["checks"] if check["name"] == name)


def _record(ledger, identifier="1"):
    return next(
        item
        for item in _check(ledger)["catch_record"]
        if item["block_run_id"] == identifier
    )


def test_scorecard_counts_fires_blocks_and_runtime():
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:00:00Z", duration=90),
        raw_run(2, "cancelled", commit=A, started="2026-01-01T00:10:00Z", duration=30),
        raw_run(3, "failure", commit=A, started="2026-01-01T00:20:00Z", duration=30),
        raw_run(4, "success", commit=B, started="2026-01-01T00:30:00Z", duration=150),
    )
    scorecard = _check(build_ledger(runs, []))["scorecard"]
    assert scorecard["fires"] == 4
    assert scorecard["blocks"] == 2
    assert scorecard["judged_commits"] == 2
    assert scorecard["runtime_seconds"] == 300
    assert scorecard["runtime_minutes"] == 5.0


def test_flake_is_counted_once_per_commit():
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:00:00Z"),
        raw_run(2, "success", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(3, "failure", commit=A, started="2026-01-01T00:20:00Z"),
        raw_run(4, "cancelled", commit=A, started="2026-01-01T00:30:00Z"),
        raw_run(5, "success", commit=B, started="2026-01-01T00:40:00Z"),
        raw_run(6, "success", commit=B, started="2026-01-01T00:50:00Z"),
    )
    scorecard = _check(build_ledger(runs, []))["scorecard"]
    assert scorecard["flaky_commits"] == 1
    assert scorecard["judged_commits"] == 2
    assert scorecard["flake_rate"] == 0.5


def test_cost_per_catch_is_null_without_a_catch():
    runs = parsed_runs(raw_run(1, "failure", commit=A, duration=600))
    scorecard = _check(build_ledger(runs, []))["scorecard"]
    assert scorecard["catches"] == 0
    assert scorecard["total_cost_usd"] == 0.08
    assert scorecard["cost_per_catch_usd"] is None


def test_cost_per_catch_uses_total_check_cost():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("src/code.py", "M")]),
        make_commit(C, [B], "2026-01-01T02:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z", duration=300),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z", duration=60),
        raw_run(3, "failure", commit=C, started="2026-01-01T02:10:00Z", duration=180),
        raw_run(4, "success", commit=C, started="2026-01-01T02:20:00Z", duration=60),
    )
    scorecard = _check(build_ledger(runs, commits))["scorecard"]
    assert scorecard["runtime_seconds"] == 600
    assert scorecard["catches"] == 1
    assert scorecard["total_cost_usd"] == 0.08
    assert scorecard["cost_per_catch_usd"] == 0.08


def test_block_followed_by_flagged_fix_and_success_is_a_catch():
    commits = [
        make_commit(
            A,
            [],
            "2026-01-01T00:00:00Z",
            [("src/code.py", "A"), ("docs/guide.md", "A")],
        ),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "catch"
    assert record["flagged_paths"] == ["docs/guide.md", "src/code.py"]
    assert record["evidence"]["fix_commits"] == [B]
    assert record["evidence"]["validating_run_id"] == "2"


def test_block_followed_by_fix_outside_flagged_code_is_not_a_catch():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("docs/guide.md", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["evidence"]["reason"] == "later_success_without_flagged_fix"


def test_flagged_change_without_a_later_success_is_not_a_catch():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "failure", commit=B, started="2026-01-01T01:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert (
        record["evidence"]["reason"]
        == "flagged_change_without_validating_success"
    )


def test_block_followed_by_bare_retry_counts_against_the_check():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=A, started="2026-01-01T00:20:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "false_block"
    assert record["evidence"]["validating_run_id"] == "2"
    assert record["evidence"]["fix_commits"] == []


def test_bare_retry_takes_precedence_over_descendant_fix_evidence():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
        raw_run(3, "success", commit=A, started="2026-01-01T02:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "false_block"
    assert record["evidence"]["validating_run_id"] == "3"


def _three_outcome_ledger():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("src/code.py", "M")]),
        make_commit(C, [B], "2026-01-01T02:00:00Z", [("src/code.py", "M")]),
        make_commit(D, [C], "2026-01-01T03:00:00Z", [("src/code.py", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
        raw_run(3, "failure", commit=C, started="2026-01-01T02:10:00Z"),
        raw_run(4, "success", commit=C, started="2026-01-01T02:20:00Z"),
        raw_run(5, "failure", commit=D, started="2026-01-01T03:10:00Z"),
    )
    return build_ledger(runs, commits)


def test_false_block_rate_uses_blocks_as_the_denominator():
    scorecard = _check(_three_outcome_ledger())["scorecard"]
    assert scorecard["blocks"] == 3
    assert scorecard["false_blocks"] == 1
    assert scorecard["false_block_rate"] == 0.333333


def test_catch_counts_sum_with_all_block_outcomes():
    check = _check(_three_outcome_ledger())
    scorecard = check["scorecard"]
    assert len(check["catch_record"]) == scorecard["blocks"]
    assert (
        scorecard["catches"]
        + scorecard["false_blocks"]
        + scorecard["unattributed_blocks"]
        == scorecard["blocks"]
    )
    assert [item["outcome"] for item in check["catch_record"]] == [
        "catch",
        "false_block",
        "unattributed",
    ]


def test_block_with_no_later_outcome_is_unattributed():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
    ]
    runs = parsed_runs(raw_run(1, "failure", commit=A))
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["evidence"]["reason"] == "no_later_outcome"


def test_missing_blocked_commit_is_unattributed():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=MISSING, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=A, started="2026-01-01T00:20:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["flagged_paths"] == []
    assert record["evidence"]["reason"] == "blocked_commit_not_in_git_history"


def test_later_success_without_flagged_fix_is_unattributed():
    commits = [
        make_commit(A, [], "2026-01-01T00:00:00Z", [("src/code.py", "A")]),
        make_commit(B, [A], "2026-01-01T01:00:00Z", [("docs/guide.md", "M")]),
    ]
    runs = parsed_runs(
        raw_run(1, "failure", commit=A, started="2026-01-01T00:10:00Z"),
        raw_run(2, "success", commit=B, started="2026-01-01T01:10:00Z"),
    )
    record = _record(build_ledger(runs, commits))
    assert record["outcome"] == "unattributed"
    assert record["evidence"]["reason"] == "later_success_without_flagged_fix"


def _ledger_with_canary():
    document = json.loads(
        (ROOT / "examples" / "synthetic-canaryfield-results.json").read_text(
            encoding="utf-8"
        )
    )
    runs = parsed_runs(
        raw_run(1, "success", commit=A),
        raw_run(2, "success", check="style checks", commit=A),
    )
    return build_ledger(
        runs,
        [],
        repo="synthetic-ledger-repo",
        ci_history_sha256="a" * 64,
        git_history_sha256="b" * 64,
        canaryfield_results_sha256="c" * 64,
        canaryfield_document=document,
    )


def test_evidence_pack_carries_the_complete_check_record():
    ledger = _ledger_with_canary()
    packs = build_evidence_packs(ledger)
    check = _check(ledger)
    pack = packs[check["evidence_pack_file"]]
    assert pack["format"] == "catchledger-pack/1"
    assert pack["repo"] == ledger["repo"]
    assert pack["check"] == check["name"]
    assert pack["scorecard"] == check["scorecard"]
    assert pack["catch_record"] == check["catch_record"]
    assert pack["canaryfield_results"] == check["canaryfield_results"]
    assert pack["false_block_rate"] == check["scorecard"]["false_block_rate"]
    assert pack["pricing"] == ledger["pricing"]
    assert pack["source"] == ledger["source"]
    assert validate_evidence_pack(pack, ledger) is True


def test_pack_validation_refuses_a_tampered_scorecard():
    ledger = _ledger_with_canary()
    pack = copy.deepcopy(build_evidence_packs(ledger)["001-style-checks.json"])
    pack["scorecard"]["fires"] += 1
    with pytest.raises(PackValidationError):
        validate_evidence_pack(pack, ledger)


def test_pack_validation_refuses_tampered_evidence_and_metadata():
    ledger = _ledger_with_canary()
    original = build_evidence_packs(ledger)["001-style-checks.json"]

    variants = []
    changed = copy.deepcopy(original)
    changed["catch_record"] = [{"outcome": "catch"}]
    variants.append(changed)
    changed = copy.deepcopy(original)
    changed["canaryfield_results"]["counts"]["caught"] += 1
    variants.append(changed)
    changed = copy.deepcopy(original)
    changed["false_block_rate"] = 1.0
    variants.append(changed)
    changed = copy.deepcopy(original)
    changed["pricing"]["cost_per_minute"] = "9.999"
    variants.append(changed)
    changed = copy.deepcopy(original)
    changed["source"]["ci_history_sha256"] = "0" * 64
    variants.append(changed)
    changed = copy.deepcopy(original)
    changed["unexpected"] = True
    variants.append(changed)
    changed = copy.deepcopy(original)
    del changed["source"]
    variants.append(changed)

    for variant in variants:
        with pytest.raises(PackValidationError):
            validate_evidence_pack(variant, ledger)


def test_canaryfield_results_are_normalized_for_the_matching_check():
    document = json.loads(
        (ROOT / "examples" / "synthetic-canaryfield-results.json").read_text(
            encoding="utf-8"
        )
    )
    normalized = normalize_canary_document(document, "unit checks")
    assert normalized["source_format"] == "canaryfield-catch-table"
    assert normalized["gate"] == "unit checks"
    assert normalized["counts"] == {
        "caught": 1,
        "waved-through": 1,
        "not-applicable": 0,
        "gate-broken": 1,
    }
    assert [row["seed"] for row in normalized["results"]] == [
        "seed-00",
        "seed-01",
        "seed-02",
    ]
    assert normalized["results"][0]["planted_files"] == ["src/math.py"]


def test_canaryfield_results_are_null_without_an_exact_gate():
    document = json.loads(
        (ROOT / "examples" / "synthetic-canaryfield-results.json").read_text(
            encoding="utf-8"
        )
    )
    assert normalize_canary_document(document, "Unit Checks") is None
    assert normalize_canary_document(document, "missing checks") is None
    assert normalize_canary_document(None, "unit checks") is None


def test_malformed_canaryfield_results_are_refused():
    with pytest.raises(CatchledgerError):
        normalize_canary_document({"gates": "unit checks", "rows": []}, "unit checks")

    document = json.loads(
        (ROOT / "examples" / "synthetic-canaryfield-results.json").read_text(
            encoding="utf-8"
        )
    )
    document["rows"][0]["gates"]["unit checks"] = "unknown"
    with pytest.raises(CatchledgerError):
        normalize_canary_document(document, "unit checks")

    document = json.loads(
        (ROOT / "examples" / "synthetic-canaryfield-results.json").read_text(
            encoding="utf-8"
        )
    )
    del document["rows"][0]["gates"]["unit checks"]
    with pytest.raises(CatchledgerError):
        normalize_canary_document(document, "unit checks")
