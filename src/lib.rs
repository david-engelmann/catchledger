//! Catchledger builds conservative per check scorecards from local history.
//!
//! This crate is the Rust port of the Python package of the same
//! name. Port slice 1 carries the JSON reader with the duplicate key
//! refusal, the timestamp reading, the ledger core with conservative
//! attribution and evidence packs, the canaryfield substrate, and
//! the deterministic JSON and markdown rendering. Port slice 2 adds
//! workflow run ingestion, the repository label reading, Git log
//! parsing and clone reading, and the digest helpers. Port
//! slice 3 makes the binary the command line whole and retires
//! the Python package, as the contract records.

pub mod bigint;
pub mod canary;
pub mod dates;
pub mod decimal;
pub mod digest;
pub mod errors;
pub mod ingest;
pub mod jsonvalue;
pub mod ledger;
pub mod models;
pub mod reader;
pub mod render;
pub mod report;

pub use canary::{normalize_canary_document, validate_canary_document};
pub use dates::{format_timestamp, parse_timestamp};
pub use digest::{canonical_json_sha256, sha256_bytes};
pub use errors::{CatchledgerError, PackValidationError};
pub use ingest::{ci_repo_label, parse_ci_document, parse_git_log, read_git_history_from_clone};
pub use jsonvalue::JsonValue;
pub use ledger::{
    build_evidence_packs, build_ledger, validate_evidence_pack, LedgerInputs, LEDGER_FORMAT,
    PACK_FORMAT,
};
pub use models::{CommitChange, CommitRecord, RunRecord};
pub use reader::{read_json_document, read_json_file};
pub use render::render_json;
pub use report::render_markdown;
