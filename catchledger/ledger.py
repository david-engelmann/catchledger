"""Scorecard building, conservative attribution, and evidence packs."""

from __future__ import annotations

import copy
import re
from decimal import Decimal, InvalidOperation, ROUND_HALF_UP

from .errors import CatchledgerError, PackValidationError
from .ingest import normalize_canary_document
from .models import format_timestamp

LEDGER_FORMAT = "catchledger-ledger/1"
PACK_FORMAT = "catchledger-pack/1"
DEFAULT_COST_PER_MINUTE = "0.008"

_SIX_PLACES = Decimal("0.000001")


def _rounded(value):
    return float(value.quantize(_SIX_PLACES, rounding=ROUND_HALF_UP))


def _ratio(numerator, denominator):
    if denominator == 0:
        return None
    return _rounded(Decimal(numerator) / Decimal(denominator))


def _money(runtime_seconds, rate, catches=None):
    value = Decimal(runtime_seconds) / Decimal(60) * rate
    if catches is not None:
        value = value / Decimal(catches)
    return _rounded(value)


def _pricing(cost_per_minute):
    text = str(cost_per_minute).strip()
    try:
        rate = Decimal(text)
    except InvalidOperation as exc:
        raise CatchledgerError("the price per minute must be a decimal number") from exc
    if not rate.is_finite() or rate < 0:
        raise CatchledgerError("the price per minute must be nonnegative")
    return {"currency": "USD", "cost_per_minute": text}, rate


def _pack_filename(index, check):
    slug = re.sub(r"[^a-z0-9]+", "-", check.lower()).strip("-")
    if not slug:
        slug = "check"
    slug = slug[:48].rstrip("-") or "check"
    return f"{index:03d}-{slug}.json"


class _History:
    def __init__(self, commits):
        self.commits = list(commits)
        self.by_hash = {}
        self.index = {}
        for position, commit in enumerate(self.commits):
            if commit.commit in self.by_hash:
                raise CatchledgerError("the Git history repeats a commit")
            self.by_hash[commit.commit] = commit
            self.index[commit.commit] = position
        self._ancestor_cache = {}

    def ancestors_inclusive(self, commit_hash):
        if commit_hash not in self.by_hash:
            return frozenset()
        cached = self._ancestor_cache.get(commit_hash)
        if cached is not None:
            return cached
        found = set()
        pending = [commit_hash]
        while pending:
            current = pending.pop()
            if current in found or current not in self.by_hash:
                continue
            found.add(current)
            pending.extend(self.by_hash[current].parents)
        result = frozenset(found)
        self._ancestor_cache[commit_hash] = result
        return result

    def is_descendant(self, candidate, ancestor):
        return ancestor in self.ancestors_inclusive(candidate)

    def interval_fix_commits(self, blocked, success_commit, flagged_paths, success_started_at):
        if blocked not in self.by_hash or success_commit not in self.by_hash:
            return []
        if blocked == success_commit:
            return []
        success_ancestors = self.ancestors_inclusive(success_commit)
        if blocked not in success_ancestors:
            return []
        interval = success_ancestors - self.ancestors_inclusive(blocked)
        flagged = set(flagged_paths)
        fixes = []
        for commit_hash in interval:
            commit = self.by_hash[commit_hash]
            if flagged.intersection(commit.changed_paths) and (
                commit.committed_at <= success_started_at
            ):
                fixes.append(commit)
        return sorted(fixes, key=lambda commit: self.index[commit.commit])

    def any_descendant_flagged_change(self, blocked, flagged_paths):
        if blocked not in self.by_hash:
            return False
        flagged = set(flagged_paths)
        if not flagged:
            return False
        for commit in self.commits:
            if commit.commit == blocked:
                continue
            if self.is_descendant(commit.commit, blocked) and flagged.intersection(
                commit.changed_paths
            ):
                return True
        return False


def _attribute_block(block, later_runs, history):
    blocked_commit = history.by_hash.get(block.commit)
    flagged_paths = (
        sorted(blocked_commit.changed_paths) if blocked_commit is not None else []
    )

    def record(outcome, evidence):
        return {
            "block_run_id": block.identifier,
            "blocked_commit": block.commit,
            "started_at": format_timestamp(block.started_at),
            "outcome": outcome,
            "flagged_paths": flagged_paths,
            "evidence": evidence,
        }

    for candidate in later_runs:
        if candidate.commit == block.commit and candidate.conclusion == "success":
            return record(
                "false_block",
                {
                    "type": "same_commit_success_after_block",
                    "fix_commits": [],
                    "validating_run_id": candidate.identifier,
                    "reason": None,
                },
            )

    for candidate in later_runs:
        if candidate.conclusion != "success":
            continue
        if candidate.commit == block.commit:
            continue
        if not history.is_descendant(candidate.commit, block.commit):
            continue
        fixes = history.interval_fix_commits(
            block.commit,
            candidate.commit,
            flagged_paths,
            candidate.started_at,
        )
        if fixes:
            return record(
                "catch",
                {
                    "type": "flagged_fix_then_success",
                    "fix_commits": [commit.commit for commit in fixes],
                    "validating_run_id": candidate.identifier,
                    "reason": None,
                },
            )
        break

    if blocked_commit is None:
        reason = "blocked_commit_not_in_git_history"
    elif any(
        candidate.conclusion == "success"
        and candidate.commit != block.commit
        and history.is_descendant(candidate.commit, block.commit)
        for candidate in later_runs
    ):
        reason = "later_success_without_flagged_fix"
    elif history.any_descendant_flagged_change(block.commit, flagged_paths):
        reason = "flagged_change_without_validating_success"
    elif later_runs:
        reason = "later_outcome_without_validating_success"
    else:
        reason = "no_later_outcome"
    return record(
        "unattributed",
        {
            "type": "none",
            "fix_commits": [],
            "validating_run_id": None,
            "reason": reason,
        },
    )


def build_ledger(
    runs,
    commits,
    *,
    repo="unknown",
    cost_per_minute=DEFAULT_COST_PER_MINUTE,
    ci_history_sha256=None,
    git_history_sha256=None,
    canaryfield_results_sha256=None,
    canaryfield_document=None,
):
    """Build the JSON ready ledger from parsed runs and commits."""

    pricing, rate = _pricing(cost_per_minute)
    history = _History(commits)
    runs_by_check = {}
    for run in runs:
        runs_by_check.setdefault(run.check, []).append(run)
    for check_runs in runs_by_check.values():
        check_runs.sort(
            key=lambda run: (run.started_at, run.updated_at, run.identifier)
        )

    checks = []
    total_fires = 0
    total_blocks = 0
    total_catches = 0
    total_false_blocks = 0
    total_unattributed = 0
    total_runtime_seconds = 0

    for index, check in enumerate(sorted(runs_by_check), start=1):
        check_runs = runs_by_check[check]
        blocks = [run for run in check_runs if run.conclusion == "failure"]
        catch_record = []
        for position, run in enumerate(check_runs):
            if run.conclusion != "failure":
                continue
            catch_record.append(
                _attribute_block(run, check_runs[position + 1 :], history)
            )
        catches = sum(
            1 for item in catch_record if item["outcome"] == "catch"
        )
        false_blocks = sum(
            1 for item in catch_record if item["outcome"] == "false_block"
        )
        unattributed = sum(
            1 for item in catch_record if item["outcome"] == "unattributed"
        )
        conclusions_by_commit = {}
        for run in check_runs:
            conclusions_by_commit.setdefault(run.commit, set()).add(run.conclusion)
        flaky_commits = sum(
            1 for conclusions in conclusions_by_commit.values() if len(conclusions) > 1
        )
        runtime_seconds = sum(run.duration_seconds for run in check_runs)
        scorecard = {
            "fires": len(check_runs),
            "blocks": len(blocks),
            "judged_commits": len(conclusions_by_commit),
            "flaky_commits": flaky_commits,
            "flake_rate": _ratio(flaky_commits, len(conclusions_by_commit)),
            "runtime_seconds": runtime_seconds,
            "runtime_minutes": _rounded(Decimal(runtime_seconds) / Decimal(60)),
            "catches": catches,
            "false_blocks": false_blocks,
            "unattributed_blocks": unattributed,
            "false_block_rate": _ratio(false_blocks, len(blocks)),
            "total_cost_usd": _money(runtime_seconds, rate),
            "cost_per_catch_usd": (
                _money(runtime_seconds, rate, catches) if catches else None
            ),
        }
        checks.append(
            {
                "name": check,
                "scorecard": scorecard,
                "catch_record": catch_record,
                "canaryfield_results": normalize_canary_document(
                    canaryfield_document, check
                ),
                "evidence_pack_file": _pack_filename(index, check),
            }
        )
        total_fires += len(check_runs)
        total_blocks += len(blocks)
        total_catches += catches
        total_false_blocks += false_blocks
        total_unattributed += unattributed
        total_runtime_seconds += runtime_seconds

    source = {
        "ci_history_sha256": ci_history_sha256,
        "git_history_sha256": git_history_sha256,
        "canaryfield_results_sha256": canaryfield_results_sha256,
    }
    if runs:
        history_window = {
            "first_run_started_at": format_timestamp(
                min(run.started_at for run in runs)
            ),
            "last_run_updated_at": format_timestamp(
                max(run.updated_at for run in runs)
            ),
        }
    else:
        history_window = None
    return {
        "format": LEDGER_FORMAT,
        "repo": repo,
        "pricing": pricing,
        "source": source,
        "history_window": history_window,
        "checks": checks,
        "totals": {
            "fires": total_fires,
            "blocks": total_blocks,
            "catches": total_catches,
            "false_blocks": total_false_blocks,
            "unattributed_blocks": total_unattributed,
            "false_block_rate": _ratio(total_false_blocks, total_blocks),
            "runtime_seconds": total_runtime_seconds,
            "runtime_minutes": _rounded(
                Decimal(total_runtime_seconds) / Decimal(60)
            ),
            "total_cost_usd": _money(total_runtime_seconds, rate),
        },
    }


def build_evidence_packs(ledger):
    """Build one evidence pack per check from a ledger."""

    packs = {}
    for check in ledger["checks"]:
        scorecard = check["scorecard"]
        pack = {
            "format": PACK_FORMAT,
            "repo": ledger["repo"],
            "check": check["name"],
            "scorecard": copy.deepcopy(scorecard),
            "catch_record": copy.deepcopy(check["catch_record"]),
            "canaryfield_results": copy.deepcopy(check["canaryfield_results"]),
            "false_block_rate": scorecard["false_block_rate"],
            "history_window": copy.deepcopy(ledger["history_window"]),
            "pricing": copy.deepcopy(ledger["pricing"]),
            "source": copy.deepcopy(ledger["source"]),
        }
        packs[check["evidence_pack_file"]] = pack
    return packs


def validate_evidence_pack(pack, ledger):
    """Return True when a pack exactly matches the pack rebuilt from a ledger."""

    if not isinstance(pack, dict):
        raise PackValidationError("an evidence pack must be an object")
    check_name = pack.get("check")
    if not isinstance(check_name, str) or not check_name:
        raise PackValidationError("an evidence pack needs a check name")
    try:
        expected_packs = build_evidence_packs(ledger)
    except (KeyError, TypeError, AttributeError) as exc:
        raise PackValidationError("the ledger cannot rebuild its packs") from exc
    expected = None
    for candidate in expected_packs.values():
        if candidate["check"] == check_name:
            expected = candidate
            break
    if expected is None:
        raise PackValidationError("the evidence pack names a check outside its ledger")
    if pack != expected:
        raise PackValidationError("the evidence pack disagrees with its ledger")
    return True
