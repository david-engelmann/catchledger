"""Deterministic JSON and markdown rendering."""

from __future__ import annotations

import json

from .ingest import CANARY_RESULT_WORDS


def render_json(document):
    return json.dumps(document, ensure_ascii=False, indent=2, sort_keys=True) + "\n"


def _cell(value):
    if value is None:
        return "none"
    return str(value).replace("|", "\\|").replace("\n", " ")


def _money(value):
    if value is None:
        return "not available"
    return f"USD {value:.6f}"


def _rate(value):
    if value is None:
        return "not available"
    return f"{value:.6f}"


def _scorecard_rows(scorecard):
    return [
        ("Fires", scorecard["fires"]),
        ("Blocks", scorecard["blocks"]),
        ("Judged commits", scorecard["judged_commits"]),
        ("Flaky commits", scorecard["flaky_commits"]),
        ("Flake rate", _rate(scorecard["flake_rate"])),
        ("Runtime seconds", scorecard["runtime_seconds"]),
        ("Runtime minutes", f"{scorecard['runtime_minutes']:.6f}"),
        ("Catches", scorecard["catches"]),
        ("False blocks", scorecard["false_blocks"]),
        ("Unattributed blocks", scorecard["unattributed_blocks"]),
        ("False block rate", _rate(scorecard["false_block_rate"])),
        ("Total cost", _money(scorecard["total_cost_usd"])),
        ("Cost per catch", _money(scorecard["cost_per_catch_usd"])),
    ]


def render_markdown(ledger):
    lines = []
    lines.append("# Catchledger report")
    lines.append("")
    lines.append(f"Repository {_cell(ledger['repo'])}")
    lines.append("")
    lines.append("## Totals")
    lines.append("")
    lines.append("| Measure | Value |")
    lines.append("| --- | ---: |")
    totals = ledger["totals"]
    total_rows = [
        ("Fires", totals["fires"]),
        ("Blocks", totals["blocks"]),
        ("Catches", totals["catches"]),
        ("False blocks", totals["false_blocks"]),
        ("Unattributed blocks", totals["unattributed_blocks"]),
        ("False block rate", _rate(totals["false_block_rate"])),
        ("Runtime seconds", totals["runtime_seconds"]),
        ("Runtime minutes", f"{totals['runtime_minutes']:.6f}"),
        ("Total cost", _money(totals["total_cost_usd"])),
    ]
    for label, value in total_rows:
        lines.append(f"| {label} | {_cell(value)} |")
    lines.append("")

    for check in ledger["checks"]:
        lines.append(f"## {_cell(check['name'])}")
        lines.append("")
        lines.append("### Scorecard")
        lines.append("")
        lines.append("| Field | Value |")
        lines.append("| --- | ---: |")
        for label, value in _scorecard_rows(check["scorecard"]):
            lines.append(f"| {label} | {_cell(value)} |")
        lines.append("")
        lines.append("### Catch record")
        lines.append("")
        if not check["catch_record"]:
            lines.append("No blocks were recorded.")
        else:
            lines.append(
                "| Block run | Blocked commit | Started | Outcome | Fix commits | Validating run | Reason |"
            )
            lines.append("| --- | --- | --- | --- | --- | --- | --- |")
            for item in check["catch_record"]:
                evidence = item["evidence"]
                fix_commits = ", ".join(evidence["fix_commits"]) or "none"
                lines.append(
                    "| "
                    + " | ".join(
                        [
                            _cell(item["block_run_id"]),
                            _cell(item["blocked_commit"]),
                            _cell(item["started_at"]),
                            _cell(item["outcome"]),
                            _cell(fix_commits),
                            _cell(evidence["validating_run_id"]),
                            _cell(evidence["reason"]),
                        ]
                    )
                    + " |"
                )
        lines.append("")
        lines.append("### Canaryfield results")
        lines.append("")
        canary = check["canaryfield_results"]
        if canary is None:
            lines.append("No canaryfield results stand for this check.")
        else:
            lines.append("| Result | Count |")
            lines.append("| --- | ---: |")
            for word in CANARY_RESULT_WORDS:
                lines.append(f"| {word} | {canary['counts'][word]} |")
            lines.append("")
            lines.append("| Seed | Name | Defect class | Result | Planted files |")
            lines.append("| --- | --- | --- | --- | --- |")
            for row in canary["results"]:
                planted = ", ".join(row["planted_files"]) or "none"
                lines.append(
                    "| "
                    + " | ".join(
                        [
                            _cell(row["seed"]),
                            _cell(row["name"]),
                            _cell(row["defect_class"]),
                            _cell(row["result"]),
                            _cell(planted),
                        ]
                    )
                    + " |"
                )
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"
