//! Recorded conformance for Rust port slice 1.
//!
//! The recordings in `tests/conformance/port1_cases.json` were
//! generated from the landed Python, the spec, over the inputs its
//! own test suite uses, plus the boundary cases the contract states.
//! The recordings are never edited by hand. These tests run offline
//! and compare byte for byte or message for message.

use std::collections::BTreeMap;
use std::path::PathBuf;

use catchledger::jsonvalue::JsonValue;
use catchledger::models::{CommitChange, CommitRecord, RunRecord};
use catchledger::{
    build_evidence_packs, build_ledger, format_timestamp, normalize_canary_document,
    parse_timestamp, read_json_document, render_json, render_markdown, validate_evidence_pack,
    LedgerInputs,
};

fn recordings() -> JsonValue {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance")
        .join("port1_cases.json");
    let text = std::fs::read_to_string(&path).expect("the recordings are readable");
    read_json_document(&text, "conformance recordings").expect("the recordings decode")
}

fn all_cases(root: &JsonValue) -> &[JsonValue] {
    root.get("cases")
        .and_then(JsonValue::as_list)
        .expect("the recordings carry a cases list")
}

fn kind_cases<'a>(all: &'a [JsonValue], kind: &str) -> Vec<&'a JsonValue> {
    all.iter()
        .filter(|case| case.get("kind").and_then(JsonValue::as_text) == Some(kind))
        .collect()
}

fn text_of<'a>(value: &'a JsonValue, key: &str) -> &'a str {
    value
        .get(key)
        .and_then(JsonValue::as_text)
        .unwrap_or_else(|| panic!("expected a text field {key}"))
}

fn name_of(case: &JsonValue) -> &str {
    text_of(case, "name")
}

/// The recorded error as a class and message pair, when the case
/// expects a refusal.
fn expected_error(case: &JsonValue) -> Option<(String, String)> {
    let error = case.get("expect")?.get("error")?;
    let pair = error.as_list()?;
    if pair.len() != 2 {
        return None;
    }
    Some((
        pair[0].as_text()?.to_string(),
        pair[1].as_text()?.to_string(),
    ))
}

fn expected_ok(case: &JsonValue) -> Option<&JsonValue> {
    case.get("expect")?.get("ok")
}

fn check_reader(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let outcome = read_json_document(text_of(case, "text"), text_of(case, "label"));
    if let Some(expected) = expected_ok(case) {
        let expected_text = expected
            .as_text()
            .ok_or_else(|| format!("{name}: the recorded answer is not text"))?;
        let value = outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        if render_json(&value) != expected_text {
            return Err(format!("{name}: the rendered document differs"));
        }
        return Ok(());
    }
    let (class, message) = expected_error(case)
        .ok_or_else(|| format!("{name}: the case expects neither ok nor error"))?;
    match outcome {
        Ok(_) => Err(format!(
            "{name}: the document read when a refusal was recorded"
        )),
        Err(error) => {
            if class != "CatchledgerError" || error.message() != message {
                Err(format!(
                    "{name}: refusal was {} when {class} {message} was recorded",
                    error.message()
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[test]
fn reader_cases_match_the_recordings() {
    // The nesting boundary cases recurse thousands of levels deep in
    // the decoder, so the checks run on a thread with a large stack.
    let handle = std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(|| {
            let root = recordings();
            let cases = kind_cases(all_cases(&root), "reader");
            assert!(!cases.is_empty());
            cases
                .iter()
                .filter_map(|case| check_reader(case).err())
                .collect::<Vec<String>>()
        })
        .expect("the reader thread starts");
    let failures = handle.join().expect("the reader thread finishes");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check_timestamp(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let value = case.get("value").expect("a timestamp case carries a value");
    let outcome = parse_timestamp(value, "the timestamp");
    if let Some(expected) = expected_ok(case) {
        let expected_text = expected
            .as_text()
            .ok_or_else(|| format!("{name}: the recorded answer is not text"))?;
        let micros =
            outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        if format_timestamp(micros) != expected_text {
            return Err(format!(
                "{name}: read as {} when {expected_text} was recorded",
                format_timestamp(micros)
            ));
        }
        return Ok(());
    }
    let expected_message = case
        .get("expect")
        .and_then(|expect| expect.get("error"))
        .and_then(JsonValue::as_text)
        .ok_or_else(|| format!("{name}: the case expects neither ok nor error"))?;
    match outcome {
        Ok(micros) => Err(format!(
            "{name}: read as {} when a refusal was recorded",
            format_timestamp(micros)
        )),
        Err(error) => {
            if error.message() != expected_message {
                Err(format!(
                    "{name}: refusal was {} when {expected_message} was recorded",
                    error.message()
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[test]
fn timestamp_cases_match_the_recordings() {
    let root = recordings();
    let cases = kind_cases(all_cases(&root), "timestamp");
    assert!(!cases.is_empty());
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_timestamp(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn run_from(enc: &JsonValue) -> RunRecord {
    let stamp = |key: &str, field: &str| {
        parse_timestamp(enc.get(key).expect("a run carries its timestamps"), field)
            .expect("a recorded timestamp reads")
    };
    RunRecord {
        identifier: text_of(enc, "identifier").to_string(),
        check: text_of(enc, "check").to_string(),
        commit: text_of(enc, "commit").to_string(),
        conclusion: text_of(enc, "conclusion").to_string(),
        created_at: stamp("created_at", "created_at"),
        started_at: stamp("started_at", "started_at"),
        updated_at: stamp("updated_at", "updated_at"),
    }
}

fn commit_from(enc: &JsonValue) -> CommitRecord {
    let parents = enc
        .get("parents")
        .and_then(JsonValue::as_list)
        .expect("a commit carries parents")
        .iter()
        .map(|parent| parent.as_text().expect("a parent is text").to_string())
        .collect();
    let changes = enc
        .get("changes")
        .and_then(JsonValue::as_list)
        .expect("a commit carries changes")
        .iter()
        .map(|change| CommitChange {
            status: text_of(change, "status").to_string(),
            paths: change
                .get("paths")
                .and_then(JsonValue::as_list)
                .expect("a change carries paths")
                .iter()
                .map(|path| path.as_text().expect("a path is text").to_string())
                .collect(),
        })
        .collect();
    CommitRecord {
        commit: text_of(enc, "commit").to_string(),
        parents,
        committed_at: parse_timestamp(
            enc.get("committed_at").expect("a commit carries its date"),
            "committed_at",
        )
        .expect("a recorded timestamp reads"),
        subject: text_of(enc, "subject").to_string(),
        changes,
    }
}

fn digest_of(case: &JsonValue, key: &str) -> Option<String> {
    match case.get("digests")?.get(key)? {
        JsonValue::Text(text) => Some(text.clone()),
        _ => None,
    }
}

fn check_ledger(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let runs: Vec<RunRecord> = case
        .get("runs")
        .and_then(JsonValue::as_list)
        .expect("a ledger case carries runs")
        .iter()
        .map(run_from)
        .collect();
    let commits: Vec<CommitRecord> = case
        .get("commits")
        .and_then(JsonValue::as_list)
        .expect("a ledger case carries commits")
        .iter()
        .map(commit_from)
        .collect();
    let canary = match case.get("canary") {
        Some(JsonValue::Null) | None => None,
        Some(document) => Some(document.clone()),
    };
    let inputs = LedgerInputs {
        repo: text_of(case, "repo").to_string(),
        cost_per_minute: text_of(case, "cost_per_minute").to_string(),
        ci_history_sha256: digest_of(case, "ci"),
        git_history_sha256: digest_of(case, "git"),
        canaryfield_results_sha256: digest_of(case, "canary"),
        canaryfield_document: canary,
    };
    let outcome = build_ledger(&runs, &commits, &inputs);
    if let Some(expected) = expected_ok(case) {
        let ledger =
            outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        let expected_ledger = expected
            .get("ledger")
            .and_then(JsonValue::as_text)
            .ok_or_else(|| format!("{name}: no recorded ledger"))?;
        if render_json(&ledger) != expected_ledger {
            return Err(format!("{name}: the ledger document differs"));
        }
        let expected_markdown = expected
            .get("markdown")
            .and_then(JsonValue::as_text)
            .ok_or_else(|| format!("{name}: no recorded report"))?;
        let markdown = render_markdown(&ledger)
            .map_err(|error| format!("{name}: the report refused with {}", error.message()))?;
        if markdown != expected_markdown {
            return Err(format!("{name}: the markdown report differs"));
        }
        let packs = build_evidence_packs(&ledger)
            .map_err(|error| format!("{name}: the packs refused with {}", error.message()))?;
        let mut actual_packs: BTreeMap<String, String> = BTreeMap::new();
        for (filename, pack) in &packs {
            let verdict = validate_evidence_pack(pack, &ledger).map_err(|error| {
                format!("{name}: a built pack refused with {}", error.message())
            })?;
            if !verdict {
                return Err(format!("{name}: a built pack did not validate"));
            }
            actual_packs.insert(filename.clone(), render_json(pack));
        }
        let recorded_packs = expected
            .get("packs")
            .and_then(JsonValue::as_object)
            .ok_or_else(|| format!("{name}: no recorded packs"))?;
        let mut expected_packs: BTreeMap<String, String> = BTreeMap::new();
        for (filename, rendered) in recorded_packs {
            expected_packs.insert(
                filename.clone(),
                rendered
                    .as_text()
                    .ok_or_else(|| format!("{name}: a recorded pack is not text"))?
                    .to_string(),
            );
        }
        if actual_packs != expected_packs {
            return Err(format!("{name}: the evidence packs differ"));
        }
        return Ok(());
    }
    let (class, message) = expected_error(case)
        .ok_or_else(|| format!("{name}: the case expects neither ok nor error"))?;
    match outcome {
        Ok(_) => Err(format!(
            "{name}: the ledger built when a refusal was recorded"
        )),
        Err(error) => {
            if class != "CatchledgerError" || error.message() != message {
                Err(format!(
                    "{name}: refusal was {} when {class} {message} was recorded",
                    error.message()
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[test]
fn ledger_cases_match_the_recordings() {
    let root = recordings();
    let cases = kind_cases(all_cases(&root), "ledger");
    assert!(!cases.is_empty());
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_ledger(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check_pack_validate(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let pack = case.get("pack").expect("a pack case carries a pack");
    let ledger = case.get("ledger").expect("a pack case carries a ledger");
    let outcome = validate_evidence_pack(pack, ledger);
    if let Some(expected) = expected_ok(case) {
        let verdict =
            outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        if expected != &JsonValue::Bool(true) || !verdict {
            return Err(format!("{name}: the pack did not validate"));
        }
        return Ok(());
    }
    let (class, message) = expected_error(case)
        .ok_or_else(|| format!("{name}: the case expects neither ok nor error"))?;
    match outcome {
        Ok(_) => Err(format!(
            "{name}: the pack validated when a refusal was recorded"
        )),
        Err(error) => {
            if class != "PackValidationError" || error.message() != message {
                Err(format!(
                    "{name}: refusal was {} when {class} {message} was recorded",
                    error.message()
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[test]
fn pack_validation_cases_match_the_recordings() {
    let root = recordings();
    let cases = kind_cases(all_cases(&root), "pack_validate");
    assert!(!cases.is_empty());
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_pack_validate(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check_canary(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let document = match case.get("document") {
        Some(JsonValue::Null) | None => None,
        Some(document) => Some(document.clone()),
    };
    let outcome = normalize_canary_document(document.as_ref(), text_of(case, "check"));
    if let Some(expected) = expected_ok(case) {
        let normalized =
            outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        match expected {
            JsonValue::Null => {
                if normalized != JsonValue::Null {
                    return Err(format!("{name}: normalized when null was recorded"));
                }
            }
            JsonValue::Text(expected_text) => {
                if render_json(&normalized) != *expected_text {
                    return Err(format!("{name}: the normalized document differs"));
                }
            }
            _ => return Err(format!("{name}: the recorded answer has a bad shape")),
        }
        return Ok(());
    }
    let (class, message) = expected_error(case)
        .ok_or_else(|| format!("{name}: the case expects neither ok nor error"))?;
    match outcome {
        Ok(_) => Err(format!("{name}: normalized when a refusal was recorded")),
        Err(error) => {
            if class != "CatchledgerError" || error.message() != message {
                Err(format!(
                    "{name}: refusal was {} when {class} {message} was recorded",
                    error.message()
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[test]
fn canary_cases_match_the_recordings() {
    let root = recordings();
    let cases = kind_cases(all_cases(&root), "canary");
    assert!(!cases.is_empty());
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_canary(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check_render(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let document = case
        .get("document")
        .expect("a render case carries a document");
    let outcome = render_markdown(document);
    if let Some(expected) = expected_ok(case) {
        let expected_text = expected
            .as_text()
            .ok_or_else(|| format!("{name}: the recorded answer is not text"))?;
        let markdown =
            outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        if markdown != expected_text {
            return Err(format!("{name}: the markdown report differs"));
        }
        return Ok(());
    }
    let (class, message) = expected_error(case)
        .ok_or_else(|| format!("{name}: the case expects neither ok nor error"))?;
    match outcome {
        Ok(_) => Err(format!(
            "{name}: the report rendered when a refusal was recorded"
        )),
        Err(error) => {
            if class != "CatchledgerError" || error.message() != message {
                Err(format!(
                    "{name}: refusal was {} when {class} {message} was recorded",
                    error.message()
                ))
            } else {
                Ok(())
            }
        }
    }
}

#[test]
fn render_cases_match_the_recordings() {
    let root = recordings();
    let cases = kind_cases(all_cases(&root), "render");
    assert!(!cases.is_empty());
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_render(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_recorded_case_is_consumed() {
    let root = recordings();
    let all = all_cases(&root);
    let declared = match root.get("case_count") {
        Some(JsonValue::Int(count)) => *count as usize,
        _ => panic!("the recordings carry no case count"),
    };
    assert_eq!(declared, all.len());
    let consumed: usize = [
        "reader",
        "timestamp",
        "ledger",
        "pack_validate",
        "canary",
        "render",
    ]
    .iter()
    .map(|kind| kind_cases(all, kind).len())
    .sum();
    assert_eq!(consumed, all.len());
}
