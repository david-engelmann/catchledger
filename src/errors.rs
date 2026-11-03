//! Error types mirroring the Python package error classes.

use std::fmt;

/// Raised when an input or operation cannot be trusted.
///
/// Mirrors the Python `CatchledgerError`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatchledgerError {
    message: String,
}

impl CatchledgerError {
    pub fn new(message: impl Into<String>) -> Self {
        CatchledgerError {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for CatchledgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CatchledgerError {}

/// Raised when an evidence pack disagrees with its ledger.
///
/// Mirrors the Python `PackValidationError`, a subclass of the
/// catchledger error there and a sibling type here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackValidationError {
    message: String,
}

impl PackValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        PackValidationError {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PackValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PackValidationError {}
