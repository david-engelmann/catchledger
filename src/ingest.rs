//! Local ingestion for workflow runs, Git history, and the
//! repository label.
//!
//! Port slice 2 carries the readings the ingestion module still
//! held after the canaryfield substrate was ported in slice 1, the
//! workflow run document reading, the repository label reading, the
//! Git log parser, and the clone reading. The clone reading invokes
//! git itself, exactly as the Python does, because the source of the
//! reading is a live git object store.

use std::collections::HashMap;
use std::path::Path;

use crate::dates::{parse_timestamp, py_strip, py_whitespace};
use crate::digest::sha256_bytes;
use crate::errors::CatchledgerError;
use crate::jsonvalue::JsonValue;
use crate::models::{CommitChange, CommitRecord, RunRecord};

const GIT_LOG_FORMAT: &str = "commit %H%nparents %P%ndate %cI%nsubject %s";

/// Split a string on runs of Python whitespace, dropping the empty
/// parts, as the Python argument free split does.
fn py_split(text: &str) -> Vec<&str> {
    text.split(py_whitespace)
        .filter(|part| !part.is_empty())
        .collect()
}

/// Strip leading Python whitespace only, as the Python left strip
/// does.
fn py_lstrip(text: &str) -> &str {
    text.trim_start_matches(py_whitespace)
}

/// Whether a character ends a line for the Python line split.
fn is_line_boundary(ch: char) -> bool {
    matches!(
        ch,
        '\n' | '\r'
            | '\u{000b}'
            | '\u{000c}'
            | '\u{001c}'
            | '\u{001d}'
            | '\u{001e}'
            | '\u{0085}'
            | '\u{2028}'
            | '\u{2029}'
    )
}

/// Split a text into lines at the Python line boundaries, with a
/// carriage return and newline pair standing as one boundary and no
/// trailing empty line, as the Python line split does.
fn py_splitlines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if !is_line_boundary(ch) {
            continue;
        }
        lines.push(&text[start..index]);
        let mut end = index + ch.len_utf8();
        if ch == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
            chars.next();
            end += 1;
        }
        start = end;
    }
    if start < text.len() {
        lines.push(&text[start..]);
    }
    lines
}

fn required_text(record: &JsonValue, field: &str, label: &str) -> Result<String, CatchledgerError> {
    match record.get(field) {
        Some(JsonValue::Text(text)) if !py_strip(text).is_empty() => Ok(py_strip(text).to_string()),
        _ => Err(CatchledgerError::new(format!(
            "a completed workflow run needs a nonempty {label}"
        ))),
    }
}

/// Parse a workflow run export into completed run records.
///
/// Mirrors the Python `parse_ci_document`.
pub fn parse_ci_document(document: &JsonValue) -> Result<Vec<RunRecord>, CatchledgerError> {
    let raw_records: &Vec<JsonValue> = match document {
        JsonValue::List(items) => items,
        JsonValue::Object(_) => match document.get("workflow_runs") {
            Some(JsonValue::List(items)) => items,
            _ => {
                return Err(CatchledgerError::new(
                    "the workflow run export must be a list or an object with workflow runs",
                ))
            }
        },
        _ => {
            return Err(CatchledgerError::new(
                "the workflow run export must be a list or an object with workflow runs",
            ))
        }
    };

    let missing = JsonValue::Null;
    let mut runs: Vec<RunRecord> = Vec::new();
    let mut by_identifier: HashMap<String, usize> = HashMap::new();
    for raw in raw_records {
        if !matches!(raw, JsonValue::Object(_)) {
            return Err(CatchledgerError::new(
                "each workflow run record must be an object",
            ));
        }
        match raw.get("status") {
            None | Some(JsonValue::Null) => {}
            Some(JsonValue::Text(text)) if py_strip(text).to_lowercase() == "completed" => {}
            Some(_) => continue,
        }
        if matches!(raw.get("conclusion"), None | Some(JsonValue::Null)) {
            continue;
        }
        let identifier = match raw.get("id") {
            Some(JsonValue::Text(text)) => text.clone(),
            Some(JsonValue::Int(integer)) => integer.to_string(),
            Some(JsonValue::BigInt(text)) => text.clone(),
            _ => {
                return Err(CatchledgerError::new(
                    "a completed workflow run needs an identifier",
                ))
            }
        };
        let identifier = py_strip(&identifier).to_string();
        if identifier.is_empty() {
            return Err(CatchledgerError::new(
                "a completed workflow run needs an identifier",
            ));
        }
        let check = required_text(raw, "name", "check name")?;
        let commit = required_text(raw, "head_sha", "head commit")?.to_lowercase();
        let conclusion = required_text(raw, "conclusion", "conclusion")?.to_lowercase();
        let created_at = parse_timestamp(raw.get("created_at").unwrap_or(&missing), "created_at")?;
        let started_at = match raw.get("run_started_at") {
            None | Some(JsonValue::Null) => created_at,
            Some(value) => parse_timestamp(value, "run_started_at")?,
        };
        let updated_at = parse_timestamp(raw.get("updated_at").unwrap_or(&missing), "updated_at")?;
        let candidate = RunRecord {
            identifier,
            check,
            commit,
            conclusion,
            created_at,
            started_at,
            updated_at,
        };
        match by_identifier.get(&candidate.identifier) {
            Some(&index) => {
                if runs[index] != candidate {
                    return Err(CatchledgerError::new(
                        "two completed workflow runs share an identifier and disagree",
                    ));
                }
            }
            None => {
                by_identifier.insert(candidate.identifier.clone(), runs.len());
                runs.push(candidate);
            }
        }
    }
    runs.sort_by(|a, b| {
        a.check
            .cmp(&b.check)
            .then_with(|| a.started_at.cmp(&b.started_at))
            .then_with(|| a.updated_at.cmp(&b.updated_at))
            .then_with(|| a.identifier.cmp(&b.identifier))
    });
    Ok(runs)
}

/// Read the repository label out of a workflow run export document.
///
/// Mirrors the Python `ci_repo_label`, which never refuses.
pub fn ci_repo_label(document: &JsonValue) -> String {
    if matches!(document, JsonValue::Object(_)) {
        if let Some(JsonValue::Text(repo)) = document.get("repo") {
            let label = py_strip(repo);
            if !label.is_empty() {
                return label.to_string();
            }
        }
        match document.get("repository") {
            Some(JsonValue::Text(text)) => {
                let label = py_strip(text);
                if !label.is_empty() {
                    return label.to_string();
                }
            }
            Some(repository @ JsonValue::Object(_)) => {
                if let Some(JsonValue::Text(full_name)) = repository.get("full_name") {
                    let label = py_strip(full_name);
                    if !label.is_empty() {
                        return label.to_string();
                    }
                }
            }
            _ => {}
        }
    }
    "unknown".to_string()
}

struct PartialRecord {
    commit: String,
    parents: Option<Vec<String>>,
    date: Option<i128>,
    subject: Option<String>,
    changes: Vec<CommitChange>,
}

fn finish_record(
    current: Option<PartialRecord>,
    records: &mut Vec<CommitRecord>,
) -> Result<(), CatchledgerError> {
    let Some(current) = current else {
        return Ok(());
    };
    let (Some(parents), Some(committed_at), Some(subject)) =
        (current.parents, current.date, current.subject)
    else {
        return Err(CatchledgerError::new("a Git history record is incomplete"));
    };
    records.push(CommitRecord {
        commit: current.commit,
        parents,
        committed_at,
        subject,
        changes: current.changes,
    });
    Ok(())
}

/// Parse git log output in the documented catchledger shape.
///
/// Mirrors the Python `parse_git_log`.
pub fn parse_git_log(text: &str) -> Result<Vec<CommitRecord>, CatchledgerError> {
    let mut records: Vec<CommitRecord> = Vec::new();
    let mut current: Option<PartialRecord> = None;
    for line in py_splitlines(text) {
        if line.starts_with("commit ") {
            finish_record(current.take(), &mut records)?;
            let parts = py_split(line);
            if parts.len() != 2 {
                return Err(CatchledgerError::new(
                    "a Git history commit line is malformed",
                ));
            }
            current = Some(PartialRecord {
                commit: parts[1].to_lowercase(),
                parents: None,
                date: None,
                subject: None,
                changes: Vec::new(),
            });
            continue;
        }
        let Some(partial) = current.as_mut() else {
            if !py_strip(line).is_empty() {
                return Err(CatchledgerError::new(
                    "Git history data stands before a commit",
                ));
            }
            continue;
        };
        if let Some(rest) = line.strip_prefix("parents") {
            if partial.parents.is_some() {
                return Err(CatchledgerError::new(
                    "a Git history record repeats its parents",
                ));
            }
            let parent_text = py_strip(rest);
            partial.parents = Some(
                py_split(parent_text)
                    .iter()
                    .map(|parent| parent.to_lowercase())
                    .collect(),
            );
            continue;
        }
        if let Some(rest) = line.strip_prefix("date ") {
            if partial.date.is_some() {
                return Err(CatchledgerError::new(
                    "a Git history record repeats its date",
                ));
            }
            partial.date = Some(parse_timestamp(
                &JsonValue::Text(rest.to_string()),
                "commit date",
            )?);
            continue;
        }
        if let Some(rest) = line.strip_prefix("subject") {
            if partial.subject.is_some() {
                return Err(CatchledgerError::new(
                    "a Git history record repeats its subject",
                ));
            }
            partial.subject = Some(py_lstrip(rest).to_string());
            continue;
        }
        if py_strip(line).is_empty() {
            continue;
        }
        if !line.contains('\t') {
            return Err(CatchledgerError::new(
                "a Git history change line is malformed",
            ));
        }
        let parts: Vec<&str> = line.split('\t').collect();
        let status = py_strip(parts[0]);
        let paths = &parts[1..];
        if status.is_empty() || paths.iter().any(|path| path.is_empty()) {
            return Err(CatchledgerError::new(
                "a Git history change line is malformed",
            ));
        }
        partial.changes.push(CommitChange {
            status: status.to_string(),
            paths: paths.iter().map(|path| path.to_string()).collect(),
        });
    }
    finish_record(current.take(), &mut records)?;
    Ok(records)
}

/// Read a local clone through git log and return records and a
/// digest.
///
/// Mirrors the Python `read_git_history_from_clone`. The reading
/// invokes git with the same arguments the Python uses, and the
/// digest is taken over the output text after the universal newline
/// translation the Python text reading applies, as the contract
/// probes record.
pub fn read_git_history_from_clone(
    path: &Path,
) -> Result<(Vec<CommitRecord>, String), CatchledgerError> {
    if !path.is_dir() {
        return Err(CatchledgerError::new(
            "the Git repository path is not a local directory",
        ));
    }
    let pretty = format!("--pretty=format:{GIT_LOG_FORMAT}");
    let completed = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["log", "--reverse", "--topo-order", "-M", "--name-status"])
        .arg(&pretty)
        .output()
        .map_err(|_| CatchledgerError::new("the local git executable could not be started"))?;
    if !completed.status.success() {
        return Err(CatchledgerError::new(
            "the local Git repository could not be read",
        ));
    }
    let text = String::from_utf8(completed.stdout)
        .map_err(|_| CatchledgerError::new("the Git history output is not UTF-8 text"))?;
    let translated = text.replace("\r\n", "\n").replace('\r', "\n");
    let records = parse_git_log(&translated)?;
    let digest = sha256_bytes(translated.as_bytes());
    Ok((records, digest))
}
