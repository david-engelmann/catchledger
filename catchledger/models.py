"""Typed records used between ingestion and ledger building."""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime, timezone

from .errors import CatchledgerError


def parse_timestamp(value, field):
    """Parse an ISO 8601 timestamp and normalize it to UTC."""

    if not isinstance(value, str) or not value.strip():
        raise CatchledgerError(f"{field} must be an ISO 8601 timestamp")
    text = value.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = datetime.fromisoformat(text)
    except ValueError as exc:
        raise CatchledgerError(f"{field} must be an ISO 8601 timestamp") from exc
    if parsed.tzinfo is None:
        raise CatchledgerError(f"{field} must carry a timezone offset")
    return parsed.astimezone(timezone.utc)


def format_timestamp(value):
    """Render a datetime in UTC with a trailing Z."""

    return value.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


@dataclass(frozen=True)
class RunRecord:
    identifier: str
    check: str
    commit: str
    conclusion: str
    created_at: datetime
    started_at: datetime
    updated_at: datetime

    @property
    def duration_seconds(self):
        seconds = (self.updated_at - self.started_at).total_seconds()
        if seconds <= 0:
            return 0
        return int(seconds)


@dataclass(frozen=True)
class CommitChange:
    status: str
    paths: tuple[str, ...]


@dataclass(frozen=True)
class CommitRecord:
    commit: str
    parents: tuple[str, ...]
    committed_at: datetime
    subject: str
    changes: tuple[CommitChange, ...]

    @property
    def changed_paths(self):
        paths = []
        for change in self.changes:
            for path in change.paths:
                if path not in paths:
                    paths.append(path)
        return tuple(paths)
