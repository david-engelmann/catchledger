import copy

import pytest

from catchledger import CatchledgerError, parse_ci_document, parse_git_log
from catchledger.ingest import read_git_history_from_clone
from synthetic_data import create_synthetic_repo, raw_run, run_git


def test_ci_envelope_and_bare_list_ingest_the_same_runs():
    records = [
        raw_run(2, "success", started="2026-01-01T01:10:00Z"),
        raw_run(1, "failure", started="2026-01-01T00:10:00Z"),
    ]
    from_envelope = parse_ci_document({"workflow_runs": copy.deepcopy(records)})
    from_list = parse_ci_document(copy.deepcopy(records))
    assert from_envelope == from_list
    assert [run.identifier for run in from_envelope] == ["1", "2"]
    assert from_envelope[0].duration_seconds == 60


def test_incomplete_and_malformed_runs_are_not_guessed():
    incomplete = raw_run(1, None, status="in_progress")
    incomplete["conclusion"] = None
    queued = raw_run(2, None, status="queued")
    queued["conclusion"] = None
    assert parse_ci_document([incomplete, queued]) == []

    malformed = raw_run(3, "failure")
    del malformed["head_sha"]
    with pytest.raises(CatchledgerError):
        parse_ci_document([malformed])

    naive = raw_run(4, "failure")
    naive["created_at"] = "2026-01-01T00:00:00"
    with pytest.raises(CatchledgerError):
        parse_ci_document([naive])


def test_exact_duplicate_runs_count_once_and_conflicts_are_refused():
    record = raw_run(10, "failure")
    parsed = parse_ci_document([record, copy.deepcopy(record)])
    assert len(parsed) == 1

    conflict = copy.deepcopy(record)
    conflict["conclusion"] = "success"
    with pytest.raises(CatchledgerError):
        parse_ci_document([record, conflict])


def test_git_log_export_parses_records_and_renames():
    text = """commit aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
parents 
date 2026-01-01T00:00:00Z
subject Synthetic first commit
A\tdocs/guide.md
A\tsrc/code.py

commit bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
parents aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
date 2026-01-01T01:00:00Z
subject Synthetic second commit
M\tsrc/code.py
R100\tdocs/guide.md\tdocs/manual.md
"""
    records = parse_git_log(text)
    assert [record.commit for record in records] == ["a" * 40, "b" * 40]
    assert records[0].parents == ()
    assert records[1].parents == ("a" * 40,)
    assert records[0].changed_paths == ("docs/guide.md", "src/code.py")
    assert records[1].changed_paths == (
        "src/code.py",
        "docs/guide.md",
        "docs/manual.md",
    )
    assert records[1].changes[-1].status == "R100"


def test_local_clone_and_export_produce_the_same_records(tmp_path):
    repo = tmp_path / "synthetic-repo"
    first, second = create_synthetic_repo(repo)
    from_clone, _digest = read_git_history_from_clone(repo)
    export = run_git(
        repo,
        "log",
        "--reverse",
        "--topo-order",
        "-M",
        "--name-status",
        "--pretty=format:commit %H%nparents %P%ndate %cI%nsubject %s",
    )
    from_export = parse_git_log(export)
    assert from_clone == from_export
    assert [record.commit for record in from_clone] == [first, second]
    assert "docs/guide.md" in from_clone[1].changed_paths
    assert "docs/manual.md" in from_clone[1].changed_paths


def test_malformed_git_history_is_refused():
    missing_date = """commit aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
parents 
subject Synthetic commit
"""
    with pytest.raises(CatchledgerError):
        parse_git_log(missing_date)

    bad_change = """commit aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
parents 
date 2026-01-01T00:00:00Z
subject Synthetic commit
M src/code.py
"""
    with pytest.raises(CatchledgerError):
        parse_git_log(bad_change)

    with pytest.raises(CatchledgerError):
        read_git_history_from_clone("https://example.invalid/synthetic.git")
