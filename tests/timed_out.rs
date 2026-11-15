//! The pins of slice 6, the timed out result word, over
//! synthetic canaryfield documents the suite builds itself.

use catchledger::canary::{normalize_canary_document, validate_canary_document};
use catchledger::jsonvalue::JsonValue;

fn row(seed: &str, word: &str) -> JsonValue {
    JsonValue::object(vec![
        ("seed", JsonValue::Text(seed.to_string())),
        ("name", JsonValue::Text(format!("synthetic defect {seed}"))),
        (
            "defect_class",
            JsonValue::Text("synthetic class".to_string()),
        ),
        (
            "planted_files",
            JsonValue::List(vec![JsonValue::Text("src/synthetic.py".to_string())]),
        ),
        (
            "gates",
            JsonValue::object(vec![("unit checks", JsonValue::Text(word.to_string()))]),
        ),
    ])
}

fn document(last_word: &str) -> JsonValue {
    let mut rows: Vec<JsonValue> = (0..29)
        .map(|i| row(&format!("seed-{i:02}"), "caught"))
        .collect();
    rows.push(row("seed-29", last_word));
    JsonValue::object(vec![
        (
            "gates",
            JsonValue::List(vec![JsonValue::Text("unit checks".to_string())]),
        ),
        ("rows", JsonValue::List(rows)),
    ])
}

fn normalized(last_word: &str) -> JsonValue {
    normalize_canary_document(Some(&document(last_word)), "unit checks")
        .expect("the document normalizes")
}

#[test]
fn test_a_canaryfield_document_with_a_timed_out_result_is_read() {
    let document = document("timed-out");
    assert!(
        validate_canary_document(&document).is_ok(),
        "a document carrying the timed-out word validates whole"
    );
    let normalized = normalized("timed-out");
    let results = normalized
        .get("results")
        .and_then(JsonValue::as_list)
        .expect("the results stand");
    assert_eq!(results.len(), 30);
}

#[test]
fn test_a_timed_out_result_is_counted_under_no_count_word() {
    let normalized = normalized("timed-out");
    let counts = normalized.get("counts").expect("the counts stand");
    assert_eq!(counts.get("caught"), Some(&JsonValue::Int(29)));
    assert_eq!(counts.get("waved-through"), Some(&JsonValue::Int(0)));
    assert_eq!(counts.get("not-applicable"), Some(&JsonValue::Int(0)));
    assert_eq!(counts.get("gate-broken"), Some(&JsonValue::Int(0)));
    assert!(counts.get("timed-out").is_none(), "no fifth count stands");
}

#[test]
fn test_a_timed_out_word_stands_in_its_matched_row() {
    let normalized = normalized("timed-out");
    let results = normalized
        .get("results")
        .and_then(JsonValue::as_list)
        .expect("the results stand");
    let timed_out: Vec<&JsonValue> = results
        .iter()
        .filter(|row| row.get("result").and_then(JsonValue::as_text) == Some("timed-out"))
        .collect();
    assert_eq!(timed_out.len(), 1, "exactly the one row carries the word");
    assert_eq!(
        timed_out[0].get("seed").and_then(JsonValue::as_text),
        Some("seed-29")
    );
}

#[test]
fn test_an_unknown_result_word_is_still_refused() {
    let document = document("exploded");
    let error =
        validate_canary_document(&document).expect_err("an unknown word refuses the document");
    assert!(
        error.message().contains("unknown result"),
        "the refusal names the unknown result: {}",
        error.message()
    );
}
