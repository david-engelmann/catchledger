//! The canaryfield substrate from the ingestion module.
//!
//! The ledger core reads normalized canaryfield evidence for every
//! check, so the document validation and the per check normalization
//! are part of this port slice even though the rest of ingestion
//! stays Python only.

use crate::errors::CatchledgerError;
use crate::jsonvalue::JsonValue;

/// The four planted result words, in report order.
pub const CANARY_RESULT_WORDS: [&str; 4] =
    ["caught", "waved-through", "not-applicable", "gate-broken"];

/// The source shape name carried into normalized evidence.
pub const CANARY_SOURCE_FORMAT: &str = "canaryfield-catch-table";

fn gates_of(document: &JsonValue) -> Result<Vec<String>, CatchledgerError> {
    let gates = document.get("gates").and_then(JsonValue::as_list);
    match gates {
        Some(items)
            if items
                .iter()
                .all(|gate| matches!(gate, JsonValue::Text(text) if !text.is_empty())) =>
        {
            Ok(items
                .iter()
                .map(|gate| gate.as_text().expect("checked text").to_string())
                .collect())
        }
        _ => Err(CatchledgerError::new(
            "the canaryfield result needs a gates list",
        )),
    }
}

/// Validate the parts of a canaryfield catch table catchledger reads.
///
/// Mirrors `validate_canary_document` in the Python ingestion
/// module, with the same refusals in the same check order.
pub fn validate_canary_document(document: &JsonValue) -> Result<(), CatchledgerError> {
    if document.as_object().is_none() {
        return Err(CatchledgerError::new(
            "the canaryfield result must be an object",
        ));
    }
    let gates = gates_of(document)?;
    let mut seen_gates: Vec<&str> = Vec::new();
    for gate in &gates {
        if seen_gates.contains(&gate.as_str()) {
            return Err(CatchledgerError::new(
                "the canaryfield result repeats a gate",
            ));
        }
        seen_gates.push(gate.as_str());
    }
    let rows = match document.get("rows").and_then(JsonValue::as_list) {
        Some(rows) => rows,
        None => {
            return Err(CatchledgerError::new(
                "the canaryfield result needs a rows list",
            ))
        }
    };
    let mut seen_rows: Vec<(String, String)> = Vec::new();
    for row in rows {
        if row.as_object().is_none() {
            return Err(CatchledgerError::new(
                "each canaryfield row must be an object",
            ));
        }
        for field in ["seed", "name", "defect_class"] {
            match row.get(field) {
                Some(JsonValue::Text(text)) if !text.is_empty() => {}
                _ => {
                    return Err(CatchledgerError::new(
                        "each canaryfield row needs its seed data",
                    ))
                }
            }
        }
        let row_gates = match row.get("gates").and_then(JsonValue::as_object) {
            Some(gates_map) => gates_map,
            None => {
                return Err(CatchledgerError::new(
                    "each canaryfield row needs gate results",
                ))
            }
        };
        let seed = row
            .get("seed")
            .and_then(JsonValue::as_text)
            .expect("seed checked above");
        for (gate, result) in row_gates {
            if !gates.iter().any(|known| known == gate) {
                return Err(CatchledgerError::new(
                    "a canaryfield row names an unknown gate",
                ));
            }
            match result {
                JsonValue::Text(word) if CANARY_RESULT_WORDS.contains(&word.as_str()) => {}
                _ => {
                    return Err(CatchledgerError::new(
                        "a canaryfield row carries an unknown result",
                    ))
                }
            }
            let key = (seed.to_string(), gate.clone());
            if seen_rows.contains(&key) {
                return Err(CatchledgerError::new(
                    "a canaryfield result repeats a seed and gate",
                ));
            }
            seen_rows.push(key);
        }
    }
    Ok(())
}

/// Return normalized canaryfield evidence for one exact check name.
///
/// Mirrors `normalize_canary_document`, including the planted files
/// cascade of FIX-9.
pub fn normalize_canary_document(
    document: Option<&JsonValue>,
    check: &str,
) -> Result<JsonValue, CatchledgerError> {
    let document = match document {
        None => return Ok(JsonValue::Null),
        Some(document) => document,
    };
    validate_canary_document(document)?;
    let gates = gates_of(document)?;
    if !gates.iter().any(|gate| gate == check) {
        return Ok(JsonValue::Null);
    }
    let mut counts: Vec<(String, JsonValue)> = CANARY_RESULT_WORDS
        .iter()
        .map(|word| (word.to_string(), JsonValue::Int(0)))
        .collect();
    let mut results: Vec<JsonValue> = Vec::new();
    let rows = document
        .get("rows")
        .and_then(JsonValue::as_list)
        .expect("rows validated");
    for row in rows {
        let row_gates = row
            .get("gates")
            .and_then(JsonValue::as_object)
            .expect("row gates validated");
        let result = row_gates
            .iter()
            .find(|(gate, _)| gate == check)
            .map(|(_, result)| result);
        let result = match result {
            Some(JsonValue::Text(word)) => word.clone(),
            _ => {
                return Err(CatchledgerError::new(
                    "a canaryfield row lacks the matched gate result",
                ))
            }
        };
        for (word, count) in &mut counts {
            if *word == result {
                if let JsonValue::Int(value) = count {
                    *value += 1;
                }
            }
        }
        let planted_files = match row.get("planted_files") {
            Some(JsonValue::List(items))
                if items.iter().all(|item| matches!(item, JsonValue::Text(_))) =>
            {
                items.clone()
            }
            _ => match row.get("planted_file") {
                Some(JsonValue::Text(path)) => vec![JsonValue::Text(path.clone())],
                Some(JsonValue::Null) | None => Vec::new(),
                _ => {
                    return Err(CatchledgerError::new(
                        "a canaryfield row carries malformed planted files",
                    ))
                }
            },
        };
        results.push(JsonValue::object(vec![
            (
                "seed",
                JsonValue::Text(
                    row.get("seed")
                        .and_then(JsonValue::as_text)
                        .expect("seed validated")
                        .to_string(),
                ),
            ),
            (
                "name",
                JsonValue::Text(
                    row.get("name")
                        .and_then(JsonValue::as_text)
                        .expect("name validated")
                        .to_string(),
                ),
            ),
            (
                "defect_class",
                JsonValue::Text(
                    row.get("defect_class")
                        .and_then(JsonValue::as_text)
                        .expect("defect class validated")
                        .to_string(),
                ),
            ),
            ("result", JsonValue::Text(result)),
            ("planted_files", JsonValue::List(planted_files)),
        ]));
    }
    results.sort_by(|a, b| {
        let seed_a = a.get("seed").and_then(JsonValue::as_text).unwrap_or("");
        let seed_b = b.get("seed").and_then(JsonValue::as_text).unwrap_or("");
        seed_a.cmp(seed_b)
    });
    Ok(JsonValue::object(vec![
        (
            "source_format",
            JsonValue::Text(CANARY_SOURCE_FORMAT.to_string()),
        ),
        ("gate", JsonValue::Text(check.to_string())),
        ("counts", JsonValue::Object(counts)),
        ("results", JsonValue::List(results)),
    ]))
}
