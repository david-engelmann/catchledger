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
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};

use crate::dates::{parse_timestamp, py_strip, py_whitespace};
use crate::digest::sha256_bytes;
use crate::errors::CatchledgerError;
use crate::jsonvalue::JsonValue;
use crate::models::{CommitChange, CommitRecord, RunRecord};

const GIT_LOG_FORMAT: &str = "commit %H%nparents %P%ndate %cI%nsubject %s";
const CHANGE_LINE_MALFORMED: &str = "a Git history change line is malformed";
const WORKFLOW_RUN_EXPORT_SHAPE: &str =
    "the workflow run export must be a list or an object with workflow runs";
const COMPLETED_RUN_NEEDS_IDENTIFIER: &str = "a completed workflow run needs an identifier";
const LOCAL_GIT_REPOSITORY_READ: &str = "the local Git repository could not be read";

/// The allowed time of one clone reading's git subprocess, the
/// stated limit the bound of slice 7 gives the reading. The door
/// and every caller in the tree pass this limit to the reading.
pub const GIT_TIME_LIMIT: Duration = Duration::from_secs(120);

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
            _ => return Err(CatchledgerError::new(WORKFLOW_RUN_EXPORT_SHAPE)),
        },
        _ => return Err(CatchledgerError::new(WORKFLOW_RUN_EXPORT_SHAPE)),
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
            _ => return Err(CatchledgerError::new(COMPLETED_RUN_NEEDS_IDENTIFIER)),
        };
        let identifier = py_strip(&identifier).to_string();
        if identifier.is_empty() {
            return Err(CatchledgerError::new(COMPLETED_RUN_NEEDS_IDENTIFIER));
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

/// Read one path field of a git log change line. A field
/// standing wrapped in double quotes carries git's C style
/// quoting, under obligation QP-2, a backslash escape standing
/// for the quote or the backslash or a control letter or one
/// byte named in three octal digits, and the decoded bytes
/// must read as UTF-8 text. A field standing in any other
/// shape reads as it stands. A quoting that does not stand
/// refuses as a malformed change line.
fn decode_git_path(field: &str) -> Result<String, CatchledgerError> {
    let malformed = || CatchledgerError::new(CHANGE_LINE_MALFORMED);
    if !field.starts_with('"') {
        return Ok(field.to_string());
    }
    let inner = match field
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        Some(inner) if field.len() >= 2 => inner,
        _ => return Err(malformed()),
    };
    let bytes = inner.as_bytes();
    let mut decoded: Vec<u8> = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'"' {
            return Err(malformed());
        }
        if byte != b'\\' {
            decoded.push(byte);
            index += 1;
            continue;
        }
        let escaped = match bytes.get(index + 1) {
            Some(value) => *value,
            None => return Err(malformed()),
        };
        match escaped {
            b'"' => decoded.push(b'"'),
            b'\\' => decoded.push(b'\\'),
            b'n' => decoded.push(b'\n'),
            b't' => decoded.push(b'\t'),
            b'a' => decoded.push(0x07),
            b'b' => decoded.push(0x08),
            b'f' => decoded.push(0x0c),
            b'r' => decoded.push(b'\r'),
            b'v' => decoded.push(0x0b),
            b'0'..=b'7' => {
                let digits = match bytes.get(index + 1..index + 4) {
                    Some(digits)
                        if digits.len() == 3
                            && digits.iter().all(|d| (b'0'..=b'7').contains(d)) =>
                    {
                        digits
                    }
                    _ => return Err(malformed()),
                };
                let value = digits
                    .iter()
                    .fold(0u32, |acc, d| acc * 8 + u32::from(d - b'0'));
                if value > 255 {
                    return Err(malformed());
                }
                decoded.push(value as u8);
                index += 2;
            }
            _ => return Err(malformed()),
        }
        index += 2;
    }
    String::from_utf8(decoded).map_err(|_| malformed())
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
            return Err(CatchledgerError::new(CHANGE_LINE_MALFORMED));
        }
        let parts: Vec<&str> = line.split('\t').collect();
        let status = py_strip(parts[0]);
        let paths = &parts[1..];
        if status.is_empty() || paths.iter().any(|path| path.is_empty()) {
            return Err(CatchledgerError::new(CHANGE_LINE_MALFORMED));
        }
        let mut decoded_paths: Vec<String> = Vec::new();
        for path in paths {
            decoded_paths.push(decode_git_path(path)?);
        }
        if decoded_paths.iter().any(|path| path.is_empty()) {
            return Err(CatchledgerError::new(CHANGE_LINE_MALFORMED));
        }
        partial.changes.push(CommitChange {
            status: status.to_string(),
            paths: decoded_paths,
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
    allowed_time: Duration,
) -> Result<(Vec<CommitRecord>, String), CatchledgerError> {
    if !path.is_dir() {
        return Err(CatchledgerError::new(
            "the Git repository path is not a local directory",
        ));
    }
    let pretty = format!("--pretty=format:{GIT_LOG_FORMAT}");
    let mut child = std::process::Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["log", "--reverse", "--topo-order", "-M", "--name-status"])
        .arg(&pretty)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| CatchledgerError::new("the local git executable could not be started"))?;
    let mut pipe = child.stdout.take().expect("the reading pipes the output");
    let drain = std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = pipe.read_to_end(&mut buffer);
        buffer
    });
    let deadline = Instant::now() + allowed_time;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(CatchledgerError::new(
                        "the local git did not finish within the allowed time",
                    ));
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => {
                return Err(CatchledgerError::new(LOCAL_GIT_REPOSITORY_READ));
            }
        }
    };
    let stdout = drain.join().unwrap_or_default();
    if !status.success() {
        return Err(CatchledgerError::new(LOCAL_GIT_REPOSITORY_READ));
    }
    let text = String::from_utf8(stdout)
        .map_err(|_| CatchledgerError::new("the Git history output is not UTF-8 text"))?;
    let translated = text.replace("\r\n", "\n").replace('\r', "\n");
    let records = parse_git_log(&translated)?;
    let digest = sha256_bytes(translated.as_bytes());
    Ok((records, digest))
}
