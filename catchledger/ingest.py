"""Local ingestion for workflow runs, Git history, and canaryfield data."""

from __future__ import annotations

import hashlib
import json
import subprocess
from pathlib import Path

from .errors import CatchledgerError
from .models import CommitChange, CommitRecord, RunRecord, parse_timestamp

CANARY_RESULT_WORDS = (
    "caught",
    "waved-through",
    "not-applicable",
    "gate-broken",
)
CANARY_SOURCE_FORMAT = "canaryfield-catch-table"

_GIT_LOG_FORMAT = "commit %H%nparents %P%ndate %cI%nsubject %s"


def sha256_bytes(data):
    return hashlib.sha256(data).hexdigest()


def canonical_json_sha256(document):
    rendered = json.dumps(
        document,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    )
    return sha256_bytes(rendered.encode("utf-8"))


def ci_repo_label(document):
    if not isinstance(document, dict):
        return "unknown"
    repo = document.get("repo")
    if isinstance(repo, str) and repo.strip():
        return repo.strip()
    repository = document.get("repository")
    if isinstance(repository, str) and repository.strip():
        return repository.strip()
    if isinstance(repository, dict):
        full_name = repository.get("full_name")
        if isinstance(full_name, str) and full_name.strip():
            return full_name.strip()
    return "unknown"


def _required_text(record, field, label):
    value = record.get(field)
    if not isinstance(value, str) or not value.strip():
        raise CatchledgerError(f"a completed workflow run needs a nonempty {label}")
    return value.strip()


def parse_ci_document(document):
    """Parse a workflow run export into completed run records."""

    if isinstance(document, list):
        raw_records = document
    elif isinstance(document, dict) and isinstance(
        document.get("workflow_runs"), list
    ):
        raw_records = document["workflow_runs"]
    else:
        raise CatchledgerError(
            "the workflow run export must be a list or an object with workflow runs"
        )

    by_identifier = {}
    for raw in raw_records:
        if not isinstance(raw, dict):
            raise CatchledgerError("each workflow run record must be an object")
        status = raw.get("status")
        if status is not None and str(status).strip().lower() != "completed":
            continue
        if raw.get("conclusion") is None:
            continue

        identifier_value = raw.get("id")
        if isinstance(identifier_value, bool) or not isinstance(
            identifier_value, (str, int)
        ):
            raise CatchledgerError("a completed workflow run needs an identifier")
        identifier = str(identifier_value).strip()
        if not identifier:
            raise CatchledgerError("a completed workflow run needs an identifier")

        check = _required_text(raw, "name", "check name")
        commit = _required_text(raw, "head_sha", "head commit").lower()
        conclusion = _required_text(raw, "conclusion", "conclusion").lower()
        created_at = parse_timestamp(raw.get("created_at"), "created_at")
        started_value = raw.get("run_started_at")
        started_at = (
            created_at
            if started_value is None
            else parse_timestamp(started_value, "run_started_at")
        )
        updated_at = parse_timestamp(raw.get("updated_at"), "updated_at")
        candidate = RunRecord(
            identifier=identifier,
            check=check,
            commit=commit,
            conclusion=conclusion,
            created_at=created_at,
            started_at=started_at,
            updated_at=updated_at,
        )
        existing = by_identifier.get(identifier)
        if existing is not None and existing != candidate:
            raise CatchledgerError(
                "two completed workflow runs share an identifier and disagree"
            )
        if existing is None:
            by_identifier[identifier] = candidate

    return sorted(
        by_identifier.values(),
        key=lambda run: (run.check, run.started_at, run.updated_at, run.identifier),
    )


def parse_git_log(text):
    """Parse git log output in the documented catchledger shape."""

    if not isinstance(text, str):
        raise CatchledgerError("the Git history export must be text")

    records = []
    current = None

    def finish_current():
        nonlocal current
        if current is None:
            return
        missing = [
            field
            for field in ("parents", "date", "subject")
            if current[field] is None
        ]
        if missing:
            raise CatchledgerError("a Git history record is incomplete")
        records.append(
            CommitRecord(
                commit=current["commit"],
                parents=current["parents"],
                committed_at=current["date"],
                subject=current["subject"],
                changes=tuple(current["changes"]),
            )
        )
        current = None

    for line in text.splitlines():
        if line.startswith("commit "):
            finish_current()
            parts = line.split()
            if len(parts) != 2 or not parts[1]:
                raise CatchledgerError("a Git history commit line is malformed")
            current = {
                "commit": parts[1].lower(),
                "parents": None,
                "date": None,
                "subject": None,
                "changes": [],
            }
            continue
        if current is None:
            if line.strip():
                raise CatchledgerError("Git history data stands before a commit")
            continue
        if line.startswith("parents"):
            if current["parents"] is not None:
                raise CatchledgerError("a Git history record repeats its parents")
            parent_text = line[len("parents") :].strip()
            current["parents"] = tuple(
                parent.lower() for parent in parent_text.split() if parent
            )
            continue
        if line.startswith("date "):
            if current["date"] is not None:
                raise CatchledgerError("a Git history record repeats its date")
            current["date"] = parse_timestamp(line[len("date ") :], "commit date")
            continue
        if line.startswith("subject"):
            if current["subject"] is not None:
                raise CatchledgerError("a Git history record repeats its subject")
            current["subject"] = line[len("subject") :].lstrip()
            continue
        if not line.strip():
            continue
        if "\t" not in line:
            raise CatchledgerError("a Git history change line is malformed")
        parts = line.split("\t")
        status = parts[0].strip()
        paths = tuple(part for part in parts[1:])
        if not status or not paths or any(not path for path in paths):
            raise CatchledgerError("a Git history change line is malformed")
        current["changes"].append(CommitChange(status=status, paths=paths))

    finish_current()
    return records


def read_git_history_from_clone(path):
    """Read a local clone through git log and return records and a digest."""

    clone = Path(path)
    if not clone.is_dir():
        raise CatchledgerError("the Git repository path is not a local directory")
    command = [
        "git",
        "-C",
        str(clone),
        "log",
        "--reverse",
        "--topo-order",
        "-M",
        "--name-status",
        f"--pretty=format:{_GIT_LOG_FORMAT}",
    ]
    try:
        completed = subprocess.run(
            command,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
    except OSError as exc:
        raise CatchledgerError("the local git executable could not be started") from exc
    if completed.returncode != 0:
        raise CatchledgerError("the local Git repository could not be read")
    return parse_git_log(completed.stdout), sha256_bytes(
        completed.stdout.encode("utf-8")
    )


def validate_canary_document(document):
    """Validate the parts of a canaryfield catch table catchledger reads."""

    if not isinstance(document, dict):
        raise CatchledgerError("the canaryfield result must be an object")
    gates = document.get("gates")
    rows = document.get("rows")
    if not isinstance(gates, list) or any(
        not isinstance(gate, str) or not gate for gate in gates
    ):
        raise CatchledgerError("the canaryfield result needs a gates list")
    if len(set(gates)) != len(gates):
        raise CatchledgerError("the canaryfield result repeats a gate")
    if not isinstance(rows, list):
        raise CatchledgerError("the canaryfield result needs a rows list")
    seen_rows = set()
    for row in rows:
        if not isinstance(row, dict):
            raise CatchledgerError("each canaryfield row must be an object")
        for field in ("seed", "name", "defect_class"):
            if not isinstance(row.get(field), str) or not row[field]:
                raise CatchledgerError("each canaryfield row needs its seed data")
        row_gates = row.get("gates")
        if not isinstance(row_gates, dict):
            raise CatchledgerError("each canaryfield row needs gate results")
        for gate, result in row_gates.items():
            if gate not in gates:
                raise CatchledgerError("a canaryfield row names an unknown gate")
            if result not in CANARY_RESULT_WORDS:
                raise CatchledgerError("a canaryfield row carries an unknown result")
            key = (row["seed"], gate)
            if key in seen_rows:
                raise CatchledgerError("a canaryfield result repeats a seed and gate")
            seen_rows.add(key)


def normalize_canary_document(document, check):
    """Return normalized canaryfield evidence for one exact check name."""

    if document is None:
        return None
    validate_canary_document(document)
    if check not in document["gates"]:
        return None
    counts = {word: 0 for word in CANARY_RESULT_WORDS}
    results = []
    for row in document["rows"]:
        row_gates = row["gates"]
        if check not in row_gates:
            raise CatchledgerError("a canaryfield row lacks the matched gate result")
        result = row_gates[check]
        counts[result] += 1
        planted_files = row.get("planted_files")
        if isinstance(planted_files, list) and all(
            isinstance(path, str) for path in planted_files
        ):
            normalized_files = list(planted_files)
        elif isinstance(row.get("planted_file"), str):
            normalized_files = [row["planted_file"]]
        elif row.get("planted_file") is None:
            normalized_files = []
        else:
            raise CatchledgerError("a canaryfield row carries malformed planted files")
        results.append(
            {
                "seed": row["seed"],
                "name": row["name"],
                "defect_class": row["defect_class"],
                "result": result,
                "planted_files": normalized_files,
            }
        )
    results.sort(key=lambda item: item["seed"])
    return {
        "source_format": CANARY_SOURCE_FORMAT,
        "gate": check,
        "counts": counts,
        "results": results,
    }
