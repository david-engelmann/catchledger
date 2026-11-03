from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_seed_files_present():
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    licence = (ROOT / "LICENSE").read_text(encoding="utf-8")
    gitignore = (ROOT / ".gitignore").read_text(encoding="utf-8")
    assert "# catchledger" in readme
    assert "The scorecard every gate should carry" in readme
    assert "## Licence" in readme
    assert "MIT License" in licence
    assert "__pycache__/" in gitignore
    assert ".env" in gitignore


def test_readme_records_the_closed_round_count():
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    assert "Closed rounds stand at 2 of 100" in readme


def test_agents_template_and_annex():
    agents = (ROOT / "AGENTS.md").read_text(encoding="utf-8")
    marker = "<!-- repo annex below this line -->"
    assert agents.startswith("# AGENTS.md")
    assert "This repo is one of the repos in David Engelmann's 39 repo storefront perfection program" in agents
    assert marker in agents
    annex = agents.split(marker, 1)[1]
    sentences = [
        "Catchledger prices checks, secondbill prices work.",
        "This repo keeps the per-check scorecard and publishes the evidence pack format the verdict tools read.",
        "All ingestion fixtures are synthetic and tests never touch the network.",
        "The local gate is python -m pytest and cargo test once the crate exists.",
        "The closed round count stands in the README and moves only at a closure.",
    ]
    positions = [annex.index(sentence) for sentence in sentences]
    assert positions == sorted(positions)
