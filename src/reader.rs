//! Reading JSON documents under the tool's refusal discipline.
//!
//! Every refusal names the document by the label the reader was
//! given, matching the Python command line reader.

use crate::errors::CatchledgerError;
use crate::jsonvalue::{parse_json, JsonValue, ParseFailure};

/// The wording of the refusal that a document could not be
/// read, under the label the reader was given.
pub fn could_not_be_read_wording(label: &str) -> String {
    format!("the {label} could not be read")
}

/// The wording of the refusal that a document is not valid
/// JSON, under the label the reader was given.
pub fn not_valid_json_wording(label: &str) -> String {
    format!("the {label} is not valid JSON")
}

fn refusal(failure: ParseFailure, label: &str) -> CatchledgerError {
    match failure {
        ParseFailure::Invalid => CatchledgerError::new(not_valid_json_wording(label)),
        ParseFailure::DuplicateKey(key) => {
            CatchledgerError::new(format!("the {label} carries the key {key} more than once"))
        }
    }
}

/// Decode a JSON document from text under the given label.
pub fn read_json_document(text: &str, label: &str) -> Result<JsonValue, CatchledgerError> {
    parse_json(text).map_err(|failure| refusal(failure, label))
}
