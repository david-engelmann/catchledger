"""Errors raised by catchledger."""


class CatchledgerError(Exception):
    """Raised when an input or operation cannot be trusted."""


class PackValidationError(CatchledgerError):
    """Raised when an evidence pack disagrees with its ledger."""
