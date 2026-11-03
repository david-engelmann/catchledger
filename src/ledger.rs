//! Scorecard building, conservative attribution, and evidence packs.
//!
//! Mirrors the Python ledger module. Every rate and money and minute
//! value comes out of the decimal pipeline the Python runs, each
//! division and multiplication rounded to the context digit count
//! before the six decimal place half up rounding closes the value.

use std::collections::{HashMap, HashSet};

use crate::canary::normalize_canary_document;
use crate::dates::{format_timestamp, py_strip};
use crate::decimal::{
    decimal_div, decimal_mul, parse_decimal, quantize_six, quantize_six_full, DecimalValue,
    Quantized,
};
use crate::errors::{CatchledgerError, PackValidationError};
use crate::jsonvalue::JsonValue;
use crate::models::{CommitRecord, RunRecord};

pub const LEDGER_FORMAT: &str = "catchledger-ledger/1";
pub const PACK_FORMAT: &str = "catchledger-pack/1";
pub const DEFAULT_COST_PER_MINUTE: &str = "0.008";

// ---------------------------------------------------------------------
// The decimal pipeline of the Python ledger module
// ---------------------------------------------------------------------

fn pricing(cost_per_minute: &str) -> Result<(JsonValue, DecimalValue), CatchledgerError> {
    let text = py_strip(cost_per_minute).to_string();
    let rate = match parse_decimal(&text) {
        Ok(rate) => rate,
        Err(()) => {
            return Err(CatchledgerError::new(
                "the price per minute must be a decimal number",
            ))
        }
    };
    match &rate {
        DecimalValue::NonFinite => {
            return Err(CatchledgerError::new(
                "the price per minute must be nonnegative",
            ))
        }
        DecimalValue::Finite {
            negative, coeff, ..
        } => {
            if *negative && !coeff.is_zero() {
                return Err(CatchledgerError::new(
                    "the price per minute must be nonnegative",
                ));
            }
        }
    }
    let record = JsonValue::object(vec![
        ("currency", JsonValue::Text("USD".to_string())),
        ("cost_per_minute", JsonValue::Text(text)),
    ]);
    Ok((record, rate))
}

/// Close a pipeline value at six decimal places the way the Python
/// rounding step closes it, taking the stated side where that step
/// raises on the Python side and the port builds in full.
fn rounded_decimal(value: DecimalValue) -> f64 {
    match quantize_six(&value) {
        Quantized::Value(closed) => closed,
        Quantized::Overflow => quantize_six_full(&value),
    }
}

fn ratio(numerator: i64, denominator: i64) -> JsonValue {
    if denominator == 0 {
        return JsonValue::Null;
    }
    let quotient = decimal_div(
        &DecimalValue::from_i64(numerator),
        &DecimalValue::from_i64(denominator),
    );
    JsonValue::Float(rounded_decimal(quotient))
}

fn money(runtime_seconds: i64, rate: &DecimalValue, catches: Option<i64>) -> f64 {
    let minutes = decimal_div(
        &DecimalValue::from_i64(runtime_seconds),
        &DecimalValue::from_i64(60),
    );
    let mut value = decimal_mul(&minutes, rate);
    if let Some(count) = catches {
        value = decimal_div(&value, &DecimalValue::from_i64(count));
    }
    rounded_decimal(value)
}

fn runtime_minutes(runtime_seconds: i64) -> f64 {
    rounded_decimal(decimal_div(
        &DecimalValue::from_i64(runtime_seconds),
        &DecimalValue::from_i64(60),
    ))
}

// ---------------------------------------------------------------------
// Pack file names
// ---------------------------------------------------------------------

fn pack_filename(index: usize, check: &str) -> String {
    let lowered = check.to_lowercase();
    let mut slug = String::new();
    for ch in lowered.chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            slug.push(ch);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let mut slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        slug = "check".to_string();
    }
    slug = slug.chars().take(48).collect::<String>();
    slug = slug.trim_end_matches('-').to_string();
    if slug.is_empty() {
        slug = "check".to_string();
    }
    format!("{index:03}-{slug}.json")
}

// ---------------------------------------------------------------------
// History and attribution
// ---------------------------------------------------------------------

struct History<'a> {
    commits: &'a [CommitRecord],
    index: HashMap<String, usize>,
    ancestor_cache: HashMap<String, HashSet<String>>,
}

impl<'a> History<'a> {
    fn new(commits: &'a [CommitRecord]) -> Result<Self, CatchledgerError> {
        let mut index = HashMap::new();
        for (position, commit) in commits.iter().enumerate() {
            if index.contains_key(&commit.commit) {
                return Err(CatchledgerError::new("the Git history repeats a commit"));
            }
            index.insert(commit.commit.clone(), position);
        }
        Ok(History {
            commits,
            index,
            ancestor_cache: HashMap::new(),
        })
    }

    fn ancestors_inclusive(&mut self, commit_hash: &str) -> HashSet<String> {
        if !self.index.contains_key(commit_hash) {
            return HashSet::new();
        }
        if let Some(cached) = self.ancestor_cache.get(commit_hash) {
            return cached.clone();
        }
        let mut found: HashSet<String> = HashSet::new();
        let mut pending = vec![commit_hash.to_string()];
        while let Some(current) = pending.pop() {
            if found.contains(&current) {
                continue;
            }
            match self.index.get(&current) {
                Some(&position) => {
                    found.insert(current);
                    for parent in &self.commits[position].parents {
                        pending.push(parent.clone());
                    }
                }
                None => continue,
            }
        }
        self.ancestor_cache
            .insert(commit_hash.to_string(), found.clone());
        found
    }

    fn is_descendant(&mut self, candidate: &str, ancestor: &str) -> bool {
        self.ancestors_inclusive(candidate).contains(ancestor)
    }

    fn interval_fix_commits(
        &mut self,
        blocked: &str,
        success_commit: &str,
        flagged_paths: &[String],
        success_started_at: i128,
    ) -> Vec<String> {
        if !self.index.contains_key(blocked) || !self.index.contains_key(success_commit) {
            return Vec::new();
        }
        if blocked == success_commit {
            return Vec::new();
        }
        let success_ancestors = self.ancestors_inclusive(success_commit);
        if !success_ancestors.contains(blocked) {
            return Vec::new();
        }
        let blocked_ancestors = self.ancestors_inclusive(blocked);
        let interval: HashSet<&str> = success_ancestors
            .difference(&blocked_ancestors)
            .map(String::as_str)
            .collect();
        let flagged: HashSet<&str> = flagged_paths.iter().map(String::as_str).collect();
        let mut fixes = Vec::new();
        for commit in self.commits {
            if !interval.contains(commit.commit.as_str()) {
                continue;
            }
            let intersects = commit
                .changed_paths()
                .iter()
                .any(|path| flagged.contains(path.as_str()));
            if intersects && commit.committed_at <= success_started_at {
                fixes.push(commit.commit.clone());
            }
        }
        fixes
    }

    fn any_descendant_flagged_change(&mut self, blocked: &str, flagged_paths: &[String]) -> bool {
        if !self.index.contains_key(blocked) || flagged_paths.is_empty() {
            return false;
        }
        let flagged: HashSet<&str> = flagged_paths.iter().map(String::as_str).collect();
        let hashes: Vec<String> = self
            .commits
            .iter()
            .map(|commit| commit.commit.clone())
            .collect();
        for hash in hashes {
            if hash == blocked {
                continue;
            }
            if self.is_descendant(&hash, blocked) {
                let position = self.index[&hash];
                let intersects = self.commits[position]
                    .changed_paths()
                    .iter()
                    .any(|path| flagged.contains(path.as_str()));
                if intersects {
                    return true;
                }
            }
        }
        false
    }
}

fn evidence(
    kind: &str,
    fix_commits: Vec<String>,
    validating_run_id: Option<String>,
    reason: Option<String>,
) -> JsonValue {
    JsonValue::object(vec![
        ("type", JsonValue::Text(kind.to_string())),
        (
            "fix_commits",
            JsonValue::List(fix_commits.into_iter().map(JsonValue::Text).collect()),
        ),
        (
            "validating_run_id",
            match validating_run_id {
                Some(identifier) => JsonValue::Text(identifier),
                None => JsonValue::Null,
            },
        ),
        (
            "reason",
            match reason {
                Some(reason) => JsonValue::Text(reason),
                None => JsonValue::Null,
            },
        ),
    ])
}

fn attribute_block(
    block: &RunRecord,
    later_runs: &[RunRecord],
    history: &mut History,
) -> JsonValue {
    let blocked_position = history.index.get(&block.commit).copied();
    let mut flagged_paths: Vec<String> = match blocked_position {
        Some(position) => history.commits[position].changed_paths(),
        None => Vec::new(),
    };
    flagged_paths.sort();
    let record = |outcome: &str, evidence: JsonValue| {
        JsonValue::object(vec![
            ("block_run_id", JsonValue::Text(block.identifier.clone())),
            ("blocked_commit", JsonValue::Text(block.commit.clone())),
            (
                "started_at",
                JsonValue::Text(format_timestamp(block.started_at)),
            ),
            ("outcome", JsonValue::Text(outcome.to_string())),
            (
                "flagged_paths",
                JsonValue::List(
                    flagged_paths
                        .iter()
                        .map(|path| JsonValue::Text(path.clone()))
                        .collect(),
                ),
            ),
            ("evidence", evidence),
        ])
    };

    for candidate in later_runs {
        if candidate.commit == block.commit && candidate.conclusion == "success" {
            return record(
                "false_block",
                evidence(
                    "same_commit_success_after_block",
                    Vec::new(),
                    Some(candidate.identifier.clone()),
                    None,
                ),
            );
        }
    }

    for candidate in later_runs {
        if candidate.conclusion != "success" || candidate.commit == block.commit {
            continue;
        }
        if !history.is_descendant(&candidate.commit, &block.commit) {
            continue;
        }
        let fixes = history.interval_fix_commits(
            &block.commit,
            &candidate.commit,
            &flagged_paths,
            candidate.started_at,
        );
        if !fixes.is_empty() {
            return record(
                "catch",
                evidence(
                    "flagged_fix_then_success",
                    fixes,
                    Some(candidate.identifier.clone()),
                    None,
                ),
            );
        }
        break;
    }

    let reason = if blocked_position.is_none() {
        "blocked_commit_not_in_git_history"
    } else if later_runs.iter().any(|candidate| {
        candidate.conclusion == "success"
            && candidate.commit != block.commit
            && history.is_descendant(&candidate.commit, &block.commit)
    }) {
        "later_success_without_flagged_fix"
    } else if history.any_descendant_flagged_change(&block.commit, &flagged_paths) {
        "flagged_change_without_validating_success"
    } else if !later_runs.is_empty() {
        "later_outcome_without_validating_success"
    } else {
        "no_later_outcome"
    };
    record(
        "unattributed",
        evidence("none", Vec::new(), None, Some(reason.to_string())),
    )
}

// ---------------------------------------------------------------------
// Ledger building
// ---------------------------------------------------------------------

/// The non record inputs to ledger building.
pub struct LedgerInputs {
    pub repo: String,
    pub cost_per_minute: String,
    pub ci_history_sha256: Option<String>,
    pub git_history_sha256: Option<String>,
    pub canaryfield_results_sha256: Option<String>,
    pub canaryfield_document: Option<JsonValue>,
}

/// Build the JSON ready ledger from parsed runs and commits.
pub fn build_ledger(
    runs: &[RunRecord],
    commits: &[CommitRecord],
    inputs: &LedgerInputs,
) -> Result<JsonValue, CatchledgerError> {
    let (pricing_record, rate) = pricing(&inputs.cost_per_minute)?;
    let mut history = History::new(commits)?;

    let mut check_names: Vec<String> = runs.iter().map(|run| run.check.clone()).collect();
    check_names.sort();
    check_names.dedup();

    let mut totals = (0i64, 0i64, 0i64, 0i64, 0i64, 0i64);
    let mut checks: Vec<JsonValue> = Vec::new();

    for (position, check) in check_names.iter().enumerate() {
        let mut check_runs: Vec<RunRecord> = runs
            .iter()
            .filter(|run| &run.check == check)
            .cloned()
            .collect();
        check_runs.sort_by(|a, b| {
            (a.started_at, a.updated_at, &a.identifier).cmp(&(
                b.started_at,
                b.updated_at,
                &b.identifier,
            ))
        });
        let blocks = check_runs
            .iter()
            .filter(|run| run.conclusion == "failure")
            .count() as i64;
        let mut catch_record: Vec<JsonValue> = Vec::new();
        for (run_position, run) in check_runs.iter().enumerate() {
            if run.conclusion != "failure" {
                continue;
            }
            let item = attribute_block(run, &check_runs[run_position + 1..], &mut history);
            catch_record.push(item);
        }
        let catches = catch_record
            .iter()
            .filter(|item| item.get("outcome") == Some(&JsonValue::Text("catch".to_string())))
            .count() as i64;
        let false_blocks = catch_record
            .iter()
            .filter(|item| item.get("outcome") == Some(&JsonValue::Text("false_block".to_string())))
            .count() as i64;
        let unattributed = catch_record
            .iter()
            .filter(|item| {
                item.get("outcome") == Some(&JsonValue::Text("unattributed".to_string()))
            })
            .count() as i64;
        let mut conclusions_by_commit: HashMap<&str, HashSet<&str>> = HashMap::new();
        for run in &check_runs {
            conclusions_by_commit
                .entry(run.commit.as_str())
                .or_default()
                .insert(run.conclusion.as_str());
        }
        let judged_commits = conclusions_by_commit.len() as i64;
        let flaky_commits = conclusions_by_commit
            .values()
            .filter(|set| set.len() > 1)
            .count() as i64;
        let runtime_seconds: i64 = check_runs.iter().map(RunRecord::duration_seconds).sum();
        let scorecard = JsonValue::object(vec![
            ("fires", JsonValue::Int(check_runs.len() as i64)),
            ("blocks", JsonValue::Int(blocks)),
            ("judged_commits", JsonValue::Int(judged_commits)),
            ("flaky_commits", JsonValue::Int(flaky_commits)),
            ("flake_rate", ratio(flaky_commits, judged_commits)),
            ("runtime_seconds", JsonValue::Int(runtime_seconds)),
            (
                "runtime_minutes",
                JsonValue::Float(runtime_minutes(runtime_seconds)),
            ),
            ("catches", JsonValue::Int(catches)),
            ("false_blocks", JsonValue::Int(false_blocks)),
            ("unattributed_blocks", JsonValue::Int(unattributed)),
            ("false_block_rate", ratio(false_blocks, blocks)),
            (
                "total_cost_usd",
                JsonValue::Float(money(runtime_seconds, &rate, None)),
            ),
            (
                "cost_per_catch_usd",
                if catches > 0 {
                    JsonValue::Float(money(runtime_seconds, &rate, Some(catches)))
                } else {
                    JsonValue::Null
                },
            ),
        ]);
        let canary = normalize_canary_document(inputs.canaryfield_document.as_ref(), check)?;
        checks.push(JsonValue::object(vec![
            ("name", JsonValue::Text(check.clone())),
            ("scorecard", scorecard),
            ("catch_record", JsonValue::List(catch_record)),
            ("canaryfield_results", canary),
            (
                "evidence_pack_file",
                JsonValue::Text(pack_filename(position + 1, check)),
            ),
        ]));
        totals.0 += check_runs.len() as i64;
        totals.1 += blocks;
        totals.2 += catches;
        totals.3 += false_blocks;
        totals.4 += unattributed;
        totals.5 += runtime_seconds;
    }

    let source = JsonValue::object(vec![
        (
            "ci_history_sha256",
            match &inputs.ci_history_sha256 {
                Some(digest) => JsonValue::Text(digest.clone()),
                None => JsonValue::Null,
            },
        ),
        (
            "git_history_sha256",
            match &inputs.git_history_sha256 {
                Some(digest) => JsonValue::Text(digest.clone()),
                None => JsonValue::Null,
            },
        ),
        (
            "canaryfield_results_sha256",
            match &inputs.canaryfield_results_sha256 {
                Some(digest) => JsonValue::Text(digest.clone()),
                None => JsonValue::Null,
            },
        ),
    ]);
    let history_window = if runs.is_empty() {
        JsonValue::Null
    } else {
        let first = runs.iter().map(|run| run.started_at).min().expect("runs");
        let last = runs.iter().map(|run| run.updated_at).max().expect("runs");
        JsonValue::object(vec![
            (
                "first_run_started_at",
                JsonValue::Text(format_timestamp(first)),
            ),
            (
                "last_run_updated_at",
                JsonValue::Text(format_timestamp(last)),
            ),
        ])
    };
    Ok(JsonValue::object(vec![
        ("format", JsonValue::Text(LEDGER_FORMAT.to_string())),
        ("repo", JsonValue::Text(inputs.repo.clone())),
        ("pricing", pricing_record),
        ("source", source),
        ("history_window", history_window),
        ("checks", JsonValue::List(checks)),
        (
            "totals",
            JsonValue::object(vec![
                ("fires", JsonValue::Int(totals.0)),
                ("blocks", JsonValue::Int(totals.1)),
                ("catches", JsonValue::Int(totals.2)),
                ("false_blocks", JsonValue::Int(totals.3)),
                ("unattributed_blocks", JsonValue::Int(totals.4)),
                ("false_block_rate", ratio(totals.3, totals.1)),
                ("runtime_seconds", JsonValue::Int(totals.5)),
                (
                    "runtime_minutes",
                    JsonValue::Float(runtime_minutes(totals.5)),
                ),
                (
                    "total_cost_usd",
                    JsonValue::Float(money(totals.5, &rate, None)),
                ),
            ]),
        ),
    ]))
}

// ---------------------------------------------------------------------
// Evidence packs
// ---------------------------------------------------------------------

fn rebuild_error() -> PackValidationError {
    PackValidationError::new("the ledger cannot rebuild its packs")
}

/// Build one evidence pack per check from a ledger.
pub fn build_evidence_packs(
    ledger: &JsonValue,
) -> Result<Vec<(String, JsonValue)>, PackValidationError> {
    let checks = ledger
        .get("checks")
        .and_then(JsonValue::as_list)
        .ok_or_else(rebuild_error)?;
    let mut packs = Vec::new();
    for check in checks {
        if check.as_object().is_none() {
            return Err(rebuild_error());
        }
        let scorecard = check.get("scorecard").ok_or_else(rebuild_error)?;
        let false_block_rate = scorecard
            .get("false_block_rate")
            .ok_or_else(rebuild_error)?;
        let pack = JsonValue::object(vec![
            ("format", JsonValue::Text(PACK_FORMAT.to_string())),
            (
                "repo",
                ledger.get("repo").ok_or_else(rebuild_error)?.clone(),
            ),
            (
                "check",
                check.get("name").ok_or_else(rebuild_error)?.clone(),
            ),
            ("scorecard", scorecard.clone()),
            (
                "catch_record",
                check.get("catch_record").ok_or_else(rebuild_error)?.clone(),
            ),
            (
                "canaryfield_results",
                check
                    .get("canaryfield_results")
                    .ok_or_else(rebuild_error)?
                    .clone(),
            ),
            ("false_block_rate", false_block_rate.clone()),
            (
                "history_window",
                ledger
                    .get("history_window")
                    .ok_or_else(rebuild_error)?
                    .clone(),
            ),
            (
                "pricing",
                ledger.get("pricing").ok_or_else(rebuild_error)?.clone(),
            ),
            (
                "source",
                ledger.get("source").ok_or_else(rebuild_error)?.clone(),
            ),
        ]);
        let filename = check
            .get("evidence_pack_file")
            .and_then(JsonValue::as_text)
            .ok_or_else(rebuild_error)?
            .to_string();
        packs.push((filename, pack));
    }
    Ok(packs)
}

/// Return true when a pack exactly matches the pack rebuilt from a
/// ledger, with the Python pack validation refusals.
pub fn validate_evidence_pack(
    pack: &JsonValue,
    ledger: &JsonValue,
) -> Result<bool, PackValidationError> {
    if pack.as_object().is_none() {
        return Err(PackValidationError::new(
            "an evidence pack must be an object",
        ));
    }
    let check_name = match pack.get("check") {
        Some(JsonValue::Text(name)) if !name.is_empty() => name.clone(),
        _ => {
            return Err(PackValidationError::new(
                "an evidence pack needs a check name",
            ))
        }
    };
    let expected_packs = build_evidence_packs(ledger)?;
    let expected = expected_packs.iter().find(|(_, candidate)| {
        candidate.get("check") == Some(&JsonValue::Text(check_name.clone()))
    });
    let expected = match expected {
        Some((_, pack)) => pack,
        None => {
            return Err(PackValidationError::new(
                "the evidence pack names a check outside its ledger",
            ))
        }
    };
    if pack != expected {
        return Err(PackValidationError::new(
            "the evidence pack disagrees with its ledger",
        ));
    }
    Ok(true)
}
