import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def test_every_contract_test_name_exists():
    contract = (ROOT / "CONTRACT.md").read_text(encoding="utf-8")
    named = sorted(set(re.findall(r"\btest_[a-z0-9_]+\b", contract)))
    test_sources = "\n".join(
        path.read_text(encoding="utf-8")
        for path in sorted((ROOT / "tests").glob("test_*.py"))
    )
    missing = [
        name for name in named if f"def {name}(" not in test_sources
    ]
    assert missing == []


def test_contract_has_no_forbidden_dashes():
    contract = (ROOT / "CONTRACT.md").read_text(encoding="utf-8")
    assert contract.count(chr(0x2014)) == 0
    assert contract.count(chr(0x2013)) == 0
