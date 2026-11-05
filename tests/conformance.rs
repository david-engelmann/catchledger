//! Recorded conformance for the Rust port slices.
//!
//! The recordings in `tests/conformance/port1_cases.json` were
//! generated from the landed Python, the spec, over the inputs its
//! own test suite uses, plus the boundary cases the contract states.
//! The recordings in `tests/conformance/port2_cases.json` stand
//! beside them for port slice 2, generated the same way over the
//! ingestion inputs of the Python suite plus the boundary families
//! the slice 2 probes recorded. The recordings are never edited by
//! hand. These tests run offline and compare byte for byte or
//! message for message, except the clone cases of slice 2, which
//! rebuild a synthetic clone from its recorded recipe on the machine
//! running the suite and compare the reading of that clone.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

// ---- Port slice 2: ingestion and digests ----

use catchledger::render::render_canonical;
use catchledger::{
    canonical_json_sha256, ci_repo_label, parse_ci_document, parse_git_log,
    read_git_history_from_clone, sha256_bytes,
};

fn recordings2() -> JsonValue {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance")
        .join("port2_cases.json");
    let text = std::fs::read_to_string(&path).expect("the slice 2 recordings are readable");
    read_json_document(&text, "conformance recordings").expect("the recordings decode")
}

fn check_run_enc(record: &RunRecord, enc: &JsonValue, name: &str) -> Result<(), String> {
    let expected = run_from(enc);
    if *record != expected {
        return Err(format!("{name}: a run record differs"));
    }
    let duration = enc
        .get("duration_seconds")
        .and_then(|value| match value {
            JsonValue::Int(number) => Some(*number),
            _ => None,
        })
        .ok_or_else(|| format!("{name}: the recorded run carries no duration"))?;
    if record.duration_seconds() != duration {
        return Err(format!("{name}: the duration differs"));
    }
    Ok(())
}

fn check_commit_enc(record: &CommitRecord, enc: &JsonValue, name: &str) -> Result<(), String> {
    let expected = commit_from(enc);
    if *record != expected {
        return Err(format!("{name}: a commit record differs"));
    }
    Ok(())
}

fn check_ci(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let document = case
        .get("document")
        .ok_or_else(|| format!("{name}: the case carries no document"))?;
    let outcome = parse_ci_document(document);
    if let Some(expected) = expected_ok(case) {
        let runs = outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        let expected_runs = expected
            .as_list()
            .ok_or_else(|| format!("{name}: the recorded answer is not a list"))?;
        if runs.len() != expected_runs.len() {
            return Err(format!(
                "{name}: read {} runs when {} were recorded",
                runs.len(),
                expected_runs.len()
            ));
        }
        for (record, enc) in runs.iter().zip(expected_runs.iter()) {
            check_run_enc(record, enc, name)?;
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
fn ci_cases_match_the_recordings() {
    let root = recordings2();
    let cases = kind_cases(all_cases(&root), "ci");
    assert_eq!(cases.len(), 59);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_ci(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check_label(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let document = case
        .get("document")
        .ok_or_else(|| format!("{name}: the case carries no document"))?;
    let expected = expected_ok(case)
        .and_then(JsonValue::as_text)
        .ok_or_else(|| format!("{name}: the recorded label is not text"))?;
    let label = ci_repo_label(document);
    if label != expected {
        return Err(format!(
            "{name}: label read as {label} when {expected} was recorded"
        ));
    }
    Ok(())
}

#[test]
fn label_cases_match_the_recordings() {
    let root = recordings2();
    let cases = kind_cases(all_cases(&root), "label");
    assert_eq!(cases.len(), 12);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_label(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check_gitlog(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let outcome = parse_git_log(text_of(case, "text"));
    if let Some(expected) = expected_ok(case) {
        let records =
            outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        let expected_records = expected
            .as_list()
            .ok_or_else(|| format!("{name}: the recorded answer is not a list"))?;
        if records.len() != expected_records.len() {
            return Err(format!(
                "{name}: read {} records when {} were recorded",
                records.len(),
                expected_records.len()
            ));
        }
        for (record, enc) in records.iter().zip(expected_records.iter()) {
            check_commit_enc(record, enc, name)?;
        }
        return Ok(());
    }
    let (class, message) = expected_error(case)
        .ok_or_else(|| format!("{name}: the case expects neither ok nor error"))?;
    match outcome {
        Ok(_) => Err(format!("{name}: the text read when a refusal was recorded")),
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
fn gitlog_cases_match_the_recordings() {
    let root = recordings2();
    let cases = kind_cases(all_cases(&root), "gitlog");
    assert_eq!(cases.len(), 44);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_gitlog(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn hex_bytes(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() / 2);
    let bytes = text.as_bytes();
    for pair in bytes.chunks_exact(2) {
        let high = (pair[0] as char).to_digit(16).expect("hex digit");
        let low = (pair[1] as char).to_digit(16).expect("hex digit");
        out.push((high * 16 + low) as u8);
    }
    out
}

fn check_sha256(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let data = hex_bytes(text_of(case, "input_hex"));
    let expected = expected_ok(case)
        .and_then(JsonValue::as_text)
        .ok_or_else(|| format!("{name}: the recorded digest is not text"))?;
    let digest = sha256_bytes(&data);
    if digest != expected {
        return Err(format!(
            "{name}: digest read as {digest} when {expected} was recorded"
        ));
    }
    Ok(())
}

#[test]
fn sha256_cases_match_the_recordings() {
    let root = recordings2();
    let cases = kind_cases(all_cases(&root), "sha256");
    assert_eq!(cases.len(), 14);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_sha256(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn check_canonical(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let document = case
        .get("document")
        .ok_or_else(|| format!("{name}: the case carries no document"))?;
    let expected =
        expected_ok(case).ok_or_else(|| format!("{name}: the case expects no answer"))?;
    let expected_rendered = text_of(expected, "rendered");
    let rendered = render_canonical(document);
    if rendered != expected_rendered {
        return Err(format!(
            "{name}: rendered as {rendered} when {expected_rendered} was recorded"
        ));
    }
    let expected_digest = text_of(expected, "digest");
    let digest = canonical_json_sha256(document);
    if digest != expected_digest {
        return Err(format!(
            "{name}: digest read as {digest} when {expected_digest} was recorded"
        ));
    }
    Ok(())
}

#[test]
fn canonical_cases_match_the_recordings() {
    let root = recordings2();
    let cases = kind_cases(all_cases(&root), "canonical");
    assert_eq!(cases.len(), 21);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_canonical(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

struct CloneReplay {
    dir: PathBuf,
}

impl CloneReplay {
    fn git(&self, args: &[&str], date: Option<&str>, stdin: Option<&[u8]>) -> String {
        let mut command = std::process::Command::new("git");
        command
            .arg("-C")
            .arg(&self.dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "Synthetic Dev")
            .env("GIT_AUTHOR_EMAIL", "synthetic@example.invalid")
            .env("GIT_COMMITTER_NAME", "Synthetic Dev")
            .env("GIT_COMMITTER_EMAIL", "synthetic@example.invalid");
        if let Some(date) = date {
            command
                .env("GIT_AUTHOR_DATE", date)
                .env("GIT_COMMITTER_DATE", date);
        }
        if stdin.is_some() {
            command.stdin(std::process::Stdio::piped());
        }
        let mut child = command
            .stdout(std::process::Stdio::piped())
            .spawn()
            .expect("git starts");
        if let Some(bytes) = stdin {
            use std::io::Write;
            child
                .stdin
                .as_mut()
                .expect("stdin is piped")
                .write_all(bytes)
                .expect("the message writes");
        }
        let output = child.wait_with_output().expect("git finishes");
        assert!(
            output.status.success(),
            "git {:?} failed with {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}

fn replay_steps(dir: &Path, steps: &[JsonValue]) {
    let replay = CloneReplay {
        dir: dir.to_path_buf(),
    };
    for step in steps {
        let op = step
            .get("op")
            .and_then(JsonValue::as_text)
            .expect("a step carries an op");
        let date = step.get("date").and_then(JsonValue::as_text);
        match op {
            "init" => {
                replay.git(&["init", "-q", "-b", "main"], None, None);
            }
            "mkdir" => {
                std::fs::create_dir_all(dir.join(text_of(step, "path")))
                    .expect("a fixture dir is made");
            }
            "write" => {
                std::fs::write(dir.join(text_of(step, "path")), text_of(step, "text"))
                    .expect("a fixture file writes");
            }
            "write_bytes" => {
                std::fs::write(
                    dir.join(text_of(step, "path")),
                    hex_bytes(text_of(step, "hex")),
                )
                .expect("a fixture file writes");
            }
            "git" => {
                let args: Vec<&str> = step
                    .get("args")
                    .and_then(JsonValue::as_list)
                    .expect("a git step carries args")
                    .iter()
                    .map(|arg| arg.as_text().expect("an arg is text"))
                    .collect();
                replay.git(&args, date, None);
            }
            "commit_tree" => {
                replay.git(&["add", "."], None, None);
                let tree = replay.git(&["write-tree"], None, None);
                let message = hex_bytes(text_of(step, "message_hex"));
                let commit = replay.git(&["commit-tree", &tree], date, Some(&message));
                replay.git(&["update-ref", "HEAD", &commit], None, None);
            }
            "symlink" => {
                // The link stands in the fixture area, the parent of
                // the built clone, and points at a sibling by name.
                let area = dir.parent().expect("a fixture build dir has a parent");
                std::os::unix::fs::symlink(
                    area.join(text_of(step, "to")),
                    area.join(text_of(step, "path")),
                )
                .expect("the fixture link is made");
            }
            other => panic!("unknown fixture op {other}"),
        }
    }
}

fn replay_clone(case: &JsonValue) -> PathBuf {
    let name = name_of(case);
    let fixture = case.get("fixture").expect("a clone case carries a fixture");
    let style = fixture
        .get("style")
        .and_then(JsonValue::as_text)
        .expect("the fixture carries a style");
    let base =
        std::env::temp_dir().join(format!("catchledger-port2-{}-{}", std::process::id(), name));
    if base.exists() {
        std::fs::remove_dir_all(&base).expect("a stale fixture clears");
    }
    match style {
        "missing" => base,
        "file" => {
            std::fs::write(&base, "not a repo\n").expect("the file writes");
            base
        }
        "repo" => {
            std::fs::create_dir_all(&base).expect("the fixture dir is made");
            let steps = fixture
                .get("steps")
                .and_then(JsonValue::as_list)
                .expect("the fixture carries steps");
            replay_steps(&base, steps);
            base
        }
        "symlink" => {
            let built = base.join(text_of(fixture, "target"));
            std::fs::create_dir_all(&built).expect("the fixture dir is made");
            let steps = fixture
                .get("steps")
                .and_then(JsonValue::as_list)
                .expect("the fixture carries steps");
            replay_steps(&built, steps);
            let link_step = steps
                .iter()
                .find(|step| step.get("op").and_then(JsonValue::as_text) == Some("symlink"))
                .expect("a symlink fixture carries a symlink step");
            base.join(text_of(link_step, "path"))
        }
        other => panic!("unknown fixture style {other}"),
    }
}

fn check_clone(case: &JsonValue) -> Result<(), String> {
    let name = name_of(case);
    let dir = replay_clone(case);
    let outcome = read_git_history_from_clone(&dir);
    if std::fs::symlink_metadata(&dir)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
    {
        // The path read was a link into a fixture area holding the
        // built clone beside it, so clear the link and the area.
        let area = dir.parent().map(|parent| parent.to_path_buf());
        let _ = std::fs::remove_file(&dir);
        if let Some(area) = area {
            let _ = std::fs::remove_dir_all(&area);
        }
    } else {
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&dir);
    }
    if let Some(expected) = expected_ok(case) {
        let (records, digest) =
            outcome.map_err(|error| format!("{name}: refused with {}", error.message()))?;
        let expected_digest = text_of(expected, "digest");
        if digest != expected_digest {
            return Err(format!(
                "{name}: digest read as {digest} when {expected_digest} was recorded"
            ));
        }
        let expected_records = expected
            .get("records")
            .and_then(JsonValue::as_list)
            .ok_or_else(|| format!("{name}: the recorded answer carries no records"))?;
        if records.len() != expected_records.len() {
            return Err(format!(
                "{name}: read {} records when {} were recorded",
                records.len(),
                expected_records.len()
            ));
        }
        for (record, enc) in records.iter().zip(expected_records.iter()) {
            check_commit_enc(record, enc, name)?;
        }
        return Ok(());
    }
    let (class, message) = expected_error(case)
        .ok_or_else(|| format!("{name}: the case expects neither ok nor error"))?;
    match outcome {
        Ok(_) => Err(format!(
            "{name}: the clone read when a refusal was recorded"
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
fn clone_cases_match_the_recordings() {
    let root = recordings2();
    let cases = kind_cases(all_cases(&root), "clone");
    assert_eq!(cases.len(), 10);
    let failures: Vec<String> = cases
        .iter()
        .filter_map(|case| check_clone(case).err())
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_slice_2_case_is_consumed() {
    let root = recordings2();
    let all = all_cases(&root);
    let declared = match root.get("case_count") {
        Some(JsonValue::Int(count)) => *count as usize,
        _ => panic!("the recordings carry no case count"),
    };
    assert_eq!(declared, all.len());
    let consumed: usize = ["ci", "label", "gitlog", "sha256", "canonical", "clone"]
        .iter()
        .map(|kind| kind_cases(all, kind).len())
        .sum();
    assert_eq!(consumed, all.len());
}
