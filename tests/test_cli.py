import json
from pathlib import Path

from catchledger.cli import main
from synthetic_data import create_synthetic_repo, raw_run


ROOT = Path(__file__).resolve().parents[1]


def _example_args(tmp_path, name):
    output_dir = tmp_path / name
    return [
        "--ci-history",
        str(ROOT / "examples" / "synthetic-runs.json"),
        "--git-log",
        str(ROOT / "examples" / "synthetic-git-log.txt"),
        "--canaryfield-results",
        str(ROOT / "examples" / "synthetic-canaryfield-results.json"),
        "--output",
        str(output_dir / "ledger.json"),
        "--report",
        str(output_dir / "report.md"),
        "--packs-dir",
        str(output_dir / "packs"),
    ]


def _output_bytes(root):
    paths = [root / "ledger.json", root / "report.md"]
    paths.extend(sorted((root / "packs").glob("*.json")))
    return {path.name: path.read_bytes() for path in paths}


def test_cli_writes_deterministic_outputs(tmp_path):
    assert main(_example_args(tmp_path, "first")) == 0
    assert main(_example_args(tmp_path, "second")) == 0
    first = _output_bytes(tmp_path / "first")
    second = _output_bytes(tmp_path / "second")
    assert first == second
    assert set(first) == {
        "ledger.json",
        "report.md",
        "001-style-checks.json",
        "002-unit-checks.json",
    }
    ledger = json.loads(first["ledger.json"])
    assert ledger["format"] == "catchledger-ledger/1"
    assert ledger["totals"]["fires"] == 6


def test_cli_accepts_a_local_clone(tmp_path):
    repo = tmp_path / "synthetic-clone"
    first, second = create_synthetic_repo(repo)
    ci_path = tmp_path / "runs.json"
    ci_path.write_text(
        json.dumps(
            {
                "repo": "synthetic-ledger-repo",
                "workflow_runs": [
                    raw_run(1, "failure", commit=first, started="2026-01-01T00:10:00Z"),
                    raw_run(2, "success", commit=second, started="2026-01-01T01:10:00Z"),
                ],
            }
        ),
        encoding="utf-8",
    )
    output = tmp_path / "out" / "ledger.json"
    report = tmp_path / "out" / "report.md"
    packs = tmp_path / "out" / "packs"
    result = main(
        [
            "--ci-history",
            str(ci_path),
            "--git-repo",
            str(repo),
            "--output",
            str(output),
            "--report",
            str(report),
            "--packs-dir",
            str(packs),
        ]
    )
    assert result == 0
    ledger = json.loads(output.read_text(encoding="utf-8"))
    assert ledger["repo"] == "synthetic-ledger-repo"
    assert ledger["checks"][0]["scorecard"]["catches"] == 1
    assert len(list(packs.glob("*.json"))) == 1


def test_ledger_and_report_carry_no_local_paths(tmp_path):
    assert main(_example_args(tmp_path, "clean")) == 0
    root = tmp_path / "clean"
    combined = "\n".join(
        path.read_text(encoding="utf-8")
        for path in [root / "ledger.json", root / "report.md", *sorted((root / "packs").glob("*.json"))]
    )
    assert str(tmp_path) not in combined
    assert str(ROOT) not in combined


def test_missing_inputs_are_refused(tmp_path, capsys):
    missing_ci = main(
        [
            "--ci-history",
            str(tmp_path / "missing-runs.json"),
            "--git-log",
            str(ROOT / "examples" / "synthetic-git-log.txt"),
            "--output",
            str(tmp_path / "ledger.json"),
            "--report",
            str(tmp_path / "report.md"),
            "--packs-dir",
            str(tmp_path / "packs"),
        ]
    )
    assert missing_ci == 2

    missing_git = main(
        [
            "--ci-history",
            str(ROOT / "examples" / "synthetic-runs.json"),
            "--git-log",
            str(tmp_path / "missing-history.txt"),
            "--output",
            str(tmp_path / "ledger.json"),
            "--report",
            str(tmp_path / "report.md"),
            "--packs-dir",
            str(tmp_path / "packs"),
        ]
    )
    assert missing_git == 2

    missing_repo = main(
        [
            "--ci-history",
            str(ROOT / "examples" / "synthetic-runs.json"),
            "--git-repo",
            str(tmp_path / "missing-repo"),
            "--output",
            str(tmp_path / "ledger.json"),
            "--report",
            str(tmp_path / "report.md"),
            "--packs-dir",
            str(tmp_path / "packs"),
        ]
    )
    assert missing_repo == 2
    assert capsys.readouterr().err.count("catchledger error") == 3
