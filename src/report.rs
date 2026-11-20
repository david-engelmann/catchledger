//! The markdown report, byte for byte the Python report.

use crate::canary::CANARY_RESULT_WORDS;
use crate::dates::py_whitespace;
use crate::errors::CatchledgerError;
use crate::jsonvalue::JsonValue;
use crate::render::json_float_text;
use crate::token::TokenFigures;

/// The spelling of a figure that cannot be given in a cell
/// of the report, read by the rate helper and the money
/// helper at their null arms and by the two fallback sites
/// inside render_token_section, spelled once here so the
/// four places that write it cannot drift apart in
/// spelling.
const NOT_AVAILABLE_TEXT: &str = "not available";

/// The separator line of the two column tables of the
/// report, read at the totals push and at the scorecard
/// push and at the canaryfield counts push inside
/// render_markdown, spelled once here so the three
/// places that write it cannot drift apart in spelling.
const TWO_COLUMN_SEPARATOR: &str = "| --- | ---: |";

fn render_error() -> CatchledgerError {
    CatchledgerError::new("the ledger cannot be rendered")
}

fn field<'a>(value: &'a JsonValue, key: &str) -> Result<&'a JsonValue, CatchledgerError> {
    value.get(key).ok_or_else(render_error)
}

/// Spell a float the way the Python string conversion spells it in
/// a plain cell, where the special values read as words rather than
/// in the JSON spelling the document rendering uses.
fn cell_float_text(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 {
            "-inf".to_string()
        } else {
            "inf".to_string()
        };
    }
    json_float_text(value)
}

/// Spell a float the way the Python six place format spelling
/// spells it in a formatted cell, where a not a number value reads
/// as nan and the infinite values read as inf and negative inf.
fn fixed6(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 {
            "-inf".to_string()
        } else {
            "inf".to_string()
        };
    }
    format!("{value:.6}")
}

fn cell(value: &JsonValue) -> Result<String, CatchledgerError> {
    let raw = match value {
        JsonValue::Null => return Ok("none".to_string()),
        JsonValue::Text(text) => text.clone(),
        JsonValue::Int(integer) => integer.to_string(),
        JsonValue::BigInt(text) => text.clone(),
        JsonValue::Float(number) => cell_float_text(*number),
        JsonValue::Bool(flag) => {
            if *flag {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        _ => return Err(render_error()),
    };
    Ok(raw.replace('|', "\\|").replace('\n', " "))
}

fn number(value: &JsonValue) -> Result<f64, CatchledgerError> {
    match value {
        JsonValue::Float(number) => Ok(*number),
        JsonValue::Int(integer) => Ok(*integer as f64),
        JsonValue::BigInt(text) => text.parse::<f64>().map_err(|_| render_error()),
        _ => Err(render_error()),
    }
}

fn rate(value: &JsonValue) -> Result<String, CatchledgerError> {
    match value {
        JsonValue::Null => Ok(NOT_AVAILABLE_TEXT.to_string()),
        _ => Ok(fixed6(number(value)?)),
    }
}

fn money(value: &JsonValue) -> Result<String, CatchledgerError> {
    match value {
        JsonValue::Null => Ok(NOT_AVAILABLE_TEXT.to_string()),
        _ => Ok(format!("USD {}", fixed6(number(value)?))),
    }
}

fn minutes(value: &JsonValue) -> Result<String, CatchledgerError> {
    Ok(fixed6(number(value)?))
}

fn plain_int(value: &JsonValue) -> Result<String, CatchledgerError> {
    match value {
        JsonValue::Int(integer) => Ok(integer.to_string()),
        _ => Err(render_error()),
    }
}

fn scorecard_rows(scorecard: &JsonValue) -> Result<Vec<(String, String)>, CatchledgerError> {
    Ok(vec![
        ("Fires".into(), plain_int(field(scorecard, "fires")?)?),
        ("Blocks".into(), plain_int(field(scorecard, "blocks")?)?),
        (
            "Judged commits".into(),
            plain_int(field(scorecard, "judged_commits")?)?,
        ),
        (
            "Flaky commits".into(),
            plain_int(field(scorecard, "flaky_commits")?)?,
        ),
        ("Flake rate".into(), rate(field(scorecard, "flake_rate")?)?),
        (
            "Runtime seconds".into(),
            plain_int(field(scorecard, "runtime_seconds")?)?,
        ),
        (
            "Runtime minutes".into(),
            minutes(field(scorecard, "runtime_minutes")?)?,
        ),
        ("Catches".into(), plain_int(field(scorecard, "catches")?)?),
        (
            "False blocks".into(),
            plain_int(field(scorecard, "false_blocks")?)?,
        ),
        (
            "Unattributed blocks".into(),
            plain_int(field(scorecard, "unattributed_blocks")?)?,
        ),
        (
            "False block rate".into(),
            rate(field(scorecard, "false_block_rate")?)?,
        ),
        (
            "Total cost".into(),
            money(field(scorecard, "total_cost_usd")?)?,
        ),
        (
            "Cost per catch".into(),
            money(field(scorecard, "cost_per_catch_usd")?)?,
        ),
    ])
}

/// Render the markdown report for a ledger document.
pub fn render_markdown(ledger: &JsonValue) -> Result<String, CatchledgerError> {
    let mut lines: Vec<String> = Vec::new();
    lines.push("# Catchledger report".to_string());
    lines.push(String::new());
    lines.push(format!("Repository {}", cell(field(ledger, "repo")?)?));
    lines.push(String::new());
    lines.push("## Totals".to_string());
    lines.push(String::new());
    lines.push("| Measure | Value |".to_string());
    lines.push(TWO_COLUMN_SEPARATOR.to_string());
    let totals = field(ledger, "totals")?;
    let total_rows: Vec<(String, String)> = vec![
        ("Fires".into(), plain_int(field(totals, "fires")?)?),
        ("Blocks".into(), plain_int(field(totals, "blocks")?)?),
        ("Catches".into(), plain_int(field(totals, "catches")?)?),
        (
            "False blocks".into(),
            plain_int(field(totals, "false_blocks")?)?,
        ),
        (
            "Unattributed blocks".into(),
            plain_int(field(totals, "unattributed_blocks")?)?,
        ),
        (
            "False block rate".into(),
            rate(field(totals, "false_block_rate")?)?,
        ),
        (
            "Runtime seconds".into(),
            plain_int(field(totals, "runtime_seconds")?)?,
        ),
        (
            "Runtime minutes".into(),
            minutes(field(totals, "runtime_minutes")?)?,
        ),
        (
            "Total cost".into(),
            money(field(totals, "total_cost_usd")?)?,
        ),
    ];
    for (label, value) in total_rows {
        lines.push(format!("| {label} | {} |", cell(&JsonValue::Text(value))?));
    }
    lines.push(String::new());

    let checks = field(ledger, "checks")?
        .as_list()
        .ok_or_else(render_error)?;
    for check in checks {
        lines.push(format!("## {}", cell(field(check, "name")?)?));
        lines.push(String::new());
        lines.push("### Scorecard".to_string());
        lines.push(String::new());
        lines.push("| Field | Value |".to_string());
        lines.push(TWO_COLUMN_SEPARATOR.to_string());
        for (label, value) in scorecard_rows(field(check, "scorecard")?)? {
            lines.push(format!("| {label} | {} |", cell(&JsonValue::Text(value))?));
        }
        lines.push(String::new());
        lines.push("### Catch record".to_string());
        lines.push(String::new());
        let catch_record = field(check, "catch_record")?
            .as_list()
            .ok_or_else(render_error)?;
        if catch_record.is_empty() {
            lines.push("No blocks were recorded.".to_string());
        } else {
            lines.push(
                "| Block run | Blocked commit | Started | Outcome | Fix commits | Validating run | Reason |"
                    .to_string(),
            );
            lines.push("| --- | --- | --- | --- | --- | --- | --- |".to_string());
            for item in catch_record {
                let evidence = field(item, "evidence")?;
                let fix_commits = field(evidence, "fix_commits")?
                    .as_list()
                    .ok_or_else(render_error)?;
                let mut fix_texts: Vec<String> = Vec::new();
                for hash in fix_commits {
                    match hash {
                        JsonValue::Text(text) => fix_texts.push(text.clone()),
                        _ => return Err(render_error()),
                    }
                }
                let fix_joined = if fix_texts.is_empty() {
                    "none".to_string()
                } else {
                    fix_texts.join(", ")
                };
                let cells = [
                    cell(field(item, "block_run_id")?)?,
                    cell(field(item, "blocked_commit")?)?,
                    cell(field(item, "started_at")?)?,
                    cell(field(item, "outcome")?)?,
                    cell(&JsonValue::Text(fix_joined))?,
                    cell(field(evidence, "validating_run_id")?)?,
                    cell(field(evidence, "reason")?)?,
                ];
                lines.push(format!("| {} |", cells.join(" | ")));
            }
        }
        lines.push(String::new());
        lines.push("### Canaryfield results".to_string());
        lines.push(String::new());
        let canary = field(check, "canaryfield_results")?;
        if *canary == JsonValue::Null {
            lines.push("No canaryfield results stand for this check.".to_string());
        } else {
            lines.push("| Result | Count |".to_string());
            lines.push(TWO_COLUMN_SEPARATOR.to_string());
            let counts = field(canary, "counts")?;
            for word in CANARY_RESULT_WORDS {
                lines.push(format!("| {word} | {} |", plain_int(field(counts, word)?)?));
            }
            lines.push(String::new());
            lines.push("| Seed | Name | Defect class | Result | Planted files |".to_string());
            lines.push("| --- | --- | --- | --- | --- |".to_string());
            let results = field(canary, "results")?
                .as_list()
                .ok_or_else(render_error)?;
            for row in results {
                let planted = field(row, "planted_files")?
                    .as_list()
                    .ok_or_else(render_error)?;
                let mut planted_texts: Vec<String> = Vec::new();
                for path in planted {
                    match path {
                        JsonValue::Text(text) => planted_texts.push(text.clone()),
                        _ => return Err(render_error()),
                    }
                }
                let planted_joined = if planted_texts.is_empty() {
                    "none".to_string()
                } else {
                    planted_texts.join(", ")
                };
                let cells = [
                    cell(field(row, "seed")?)?,
                    cell(field(row, "name")?)?,
                    cell(field(row, "defect_class")?)?,
                    cell(field(row, "result")?)?,
                    cell(&JsonValue::Text(planted_joined))?,
                ];
                lines.push(format!("| {} |", cells.join(" | ")));
            }
        }
        lines.push(String::new());
    }

    let joined = lines.join("\n");
    let trimmed = joined.trim_end_matches(py_whitespace);
    Ok(format!("{trimmed}\n"))
}

/// The opening lines of the token section, the frame the
/// renderer emits and the frame scan inside validate_report
/// reads back, spelled once here so the two sides cannot
/// drift apart in spelling.
const TOKEN_SECTION_FRAME: [&str; 4] = [
    "## Token costs",
    "",
    "| Check | Input tokens | Output tokens | Token cost |",
    "| --- | ---: | ---: | ---: |",
];

/// Render the token costs section the report carries where a
/// token usage document stood, in the report's own table shape.
/// The section stands after the standing sections, and the
/// totals line closes it in the same words as its check lines.
pub fn render_token_section(figures: &TokenFigures) -> String {
    let mut lines: Vec<String> = TOKEN_SECTION_FRAME
        .iter()
        .map(|line| line.to_string())
        .collect();
    for check in &figures.checks {
        lines.push(format!(
            "| {} | {} | {} | {} |",
            check.name,
            check.input_tokens,
            check.output_tokens,
            money(&JsonValue::Float(check.token_cost_usd))
                .unwrap_or_else(|_| NOT_AVAILABLE_TEXT.to_string())
        ));
    }
    lines.push(format!(
        "| Totals | {} | {} | {} |",
        figures.input_tokens,
        figures.output_tokens,
        money(&JsonValue::Float(figures.token_cost_usd))
            .unwrap_or_else(|_| NOT_AVAILABLE_TEXT.to_string())
    ));
    let joined = lines.join("\n");
    format!("{joined}\n")
}

/// Build the report a run writes from the ledger it built and
/// the token figures where they stand, the markdown of the
/// ledger with the token section standing after it where the
/// figures stand. The run's write and the report check read
/// this one composition.
pub fn build_report(
    ledger: &JsonValue,
    figures: Option<&TokenFigures>,
) -> Result<String, CatchledgerError> {
    let mut report = render_markdown(ledger)?;
    if let Some(figures) = figures {
        report = format!(
            "{}\n\n{}",
            report.trim_end_matches('\n'),
            render_token_section(figures)
        );
    }
    Ok(report)
}

/// Validate a supplied report against the report the same
/// ledger and figures render, byte for byte, because a report
/// is its rendering and a respelling of it is a different
/// document.
pub fn validate_report(
    supplied: &[u8],
    ledger: &JsonValue,
    figures: Option<&TokenFigures>,
) -> Result<bool, CatchledgerError> {
    let expected = build_report(ledger, figures)?;
    if supplied != expected.as_bytes() {
        let lines: Vec<&[u8]> = supplied.split(|byte| *byte == b'\n').collect();
        let token_side = figures.is_some()
            || lines.windows(4).any(|frame| {
                frame
                    .iter()
                    .zip(TOKEN_SECTION_FRAME.iter())
                    .all(|(line, member)| *line == member.as_bytes())
            });
        if token_side {
            return Err(CatchledgerError::new(
                "the report disagrees with the ledger and the token usage",
            ));
        }
        return Err(CatchledgerError::new(
            "the report disagrees with the ledger",
        ));
    }
    Ok(true)
}
