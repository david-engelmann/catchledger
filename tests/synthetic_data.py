"""Synthetic data builders shared by the catchledger tests."""

from __future__ import annotations

import os
import subprocess
from datetime import datetime, timedelta, timezone
from pathlib import Path

from catchledger.ingest import parse_ci_document
from catchledger.models import CommitChange, CommitRecord, parse_timestamp

A = "a" * 40
B = "b" * 40
C = "c" * 40
D = "d" * 40
MISSING = "f" * 40


def _format_time(value):
    return value.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def raw_run(
    identifier,
    conclusion,
    *,
    check="unit checks",
    commit=A,
    started="2026-01-01T00:10:00Z",
    duration=60,
    status="completed",
):
    start = parse_timestamp(started, "started")
    return {
        "id": identifier,
        "name": check,
        "head_sha": commit,
        "status": status,
        "conclusion": conclusion,
        "created_at": _format_time(start - timedelta(seconds=5)),
        "run_started_at": _format_time(start),
        "updated_at": _format_time(start + timedelta(seconds=duration)),
    }


def parsed_runs(*records):
    return parse_ci_document({"repo": "synthetic-ledger-repo", "workflow_runs": list(records)})


def make_commit(commit_hash, parents, committed_at, changes, subject="Synthetic commit"):
    return CommitRecord(
        commit=commit_hash,
        parents=tuple(parents),
        committed_at=parse_timestamp(committed_at, "committed_at"),
        subject=subject,
        changes=tuple(
            CommitChange(status=status, paths=(path,)) for path, status in changes
        ),
    )


def run_git(repo, *args, date=None):
    env = os.environ.copy()
    env["GIT_AUTHOR_NAME"] = "Synthetic Dev"
    env["GIT_AUTHOR_EMAIL"] = "synthetic@example.invalid"
    env["GIT_COMMITTER_NAME"] = "Synthetic Dev"
    env["GIT_COMMITTER_EMAIL"] = "synthetic@example.invalid"
    if date is not None:
        env["GIT_AUTHOR_DATE"] = date
        env["GIT_COMMITTER_DATE"] = date
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        env=env,
    ).stdout.strip()


def create_synthetic_repo(path):
    repo = Path(path)
    repo.mkdir(parents=True)
    run_git(repo, "init", "-q", "-b", "main")
    (repo / "src").mkdir()
    (repo / "docs").mkdir()
    (repo / "src" / "code.py").write_text("value = 1\n", encoding="utf-8")
    (repo / "docs" / "guide.md").write_text("Synthetic guide.\n", encoding="utf-8")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-q", "-m", "Synthetic first commit", date="2026-01-01T00:00:00Z")
    first = run_git(repo, "rev-parse", "HEAD")
    (repo / "src" / "code.py").write_text("value = 2\n", encoding="utf-8")
    run_git(repo, "mv", "docs/guide.md", "docs/manual.md")
    run_git(repo, "add", ".")
    run_git(repo, "commit", "-q", "-m", "Synthetic second commit", date="2026-01-01T01:00:00Z")
    second = run_git(repo, "rev-parse", "HEAD")
    return first, second
