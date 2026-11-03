"""Command line entry point for catchledger."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

from .errors import CatchledgerError
from .ingest import (
    ci_repo_label,
    parse_ci_document,
    parse_git_log,
    read_git_history_from_clone,
    sha256_bytes,
    validate_canary_document,
)
from .ledger import build_evidence_packs, build_ledger, validate_evidence_pack
from .report import render_json, render_markdown


def _read_bytes(path, label):
    try:
        return Path(path).read_bytes()
    except OSError as exc:
        raise CatchledgerError(f"the {label} could not be read") from exc


def _read_json(path, label):
    data = _read_bytes(path, label)

    def reject_duplicate_keys(pairs):
        seen = set()
        for key, _value in pairs:
            if key in seen:
                raise CatchledgerError(
                    f"the {label} carries the key {key} more than once"
                )
            seen.add(key)
        return dict(pairs)

    try:
        document = json.loads(
            data.decode("utf-8"), object_pairs_hook=reject_duplicate_keys
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise CatchledgerError(f"the {label} is not valid JSON") from exc
    return document, sha256_bytes(data)


def _write_text(path, text):
    destination = Path(path)
    try:
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(text, encoding="utf-8")
    except OSError as exc:
        raise CatchledgerError("an output could not be written") from exc


def build_from_args(args):
    ci_document, ci_digest = _read_json(args.ci_history, "workflow run export")
    runs = parse_ci_document(ci_document)

    if args.git_log is not None:
        git_data = _read_bytes(args.git_log, "Git history export")
        try:
            git_text = git_data.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise CatchledgerError("the Git history export is not UTF-8 text") from exc
        commits = parse_git_log(git_text)
        git_digest = sha256_bytes(git_data)
    else:
        commits, git_digest = read_git_history_from_clone(args.git_repo)

    canary_document = None
    canary_digest = None
    if args.canaryfield_results is not None:
        canary_document, canary_digest = _read_json(
            args.canaryfield_results, "canaryfield result"
        )
        validate_canary_document(canary_document)

    repo = args.repo_label or ci_repo_label(ci_document)
    ledger = build_ledger(
        runs,
        commits,
        repo=repo,
        cost_per_minute=args.cost_per_minute,
        ci_history_sha256=ci_digest,
        git_history_sha256=git_digest,
        canaryfield_results_sha256=canary_digest,
        canaryfield_document=canary_document,
    )
    return ledger, build_evidence_packs(ledger)


def run(args):
    ledger, packs = build_from_args(args)
    _write_text(args.output, render_json(ledger))
    _write_text(args.report, render_markdown(ledger))
    packs_dir = Path(args.packs_dir)
    try:
        packs_dir.mkdir(parents=True, exist_ok=True)
    except OSError as exc:
        raise CatchledgerError("the evidence pack directory could not be created") from exc
    for filename, pack in packs.items():
        _write_text(packs_dir / filename, render_json(pack))
    for filename in packs:
        written, _digest = _read_json(packs_dir / filename, "written evidence pack")
        validate_evidence_pack(written, ledger)
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(
        prog="catchledger",
        description="Build a conservative per check scorecard from local CI and Git history.",
    )
    parser.add_argument("--ci-history", required=True, help="local workflow run export")
    source = parser.add_mutually_exclusive_group(required=True)
    source.add_argument("--git-log", help="local Git history export")
    source.add_argument("--git-repo", help="local Git repository clone")
    parser.add_argument(
        "--canaryfield-results", help="local canaryfield catch table JSON"
    )
    parser.add_argument("--output", required=True, help="JSON ledger output path")
    parser.add_argument("--report", required=True, help="markdown report output path")
    parser.add_argument(
        "--packs-dir", required=True, help="directory for evidence pack files"
    )
    parser.add_argument("--repo-label", help="repository label for the outputs")
    parser.add_argument(
        "--cost-per-minute",
        default="0.008",
        help="price per runtime minute in USD",
    )
    args = parser.parse_args(argv)
    try:
        return run(args)
    except CatchledgerError as exc:
        print(f"catchledger error: {exc}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
