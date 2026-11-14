//! The token usage join of slice 4.
//!
//! A token usage document under the format name
//! catchledger-token-usage/1 names the token counts of workflow
//! runs by the identifier the run records carry. The join sums
//! the counts per check and prices them through the decimal
//! pipeline the ledger prices runtime through, and the figures
//! stand in a token cost document of their own format, so the
//! ledger and the evidence packs keep the exact fields the
//! earlier slices gave them.

use std::collections::HashMap;

use crate::decimal::{
    decimal_add, decimal_mul, quantize_six, quantize_six_full, DecimalValue, Quantized,
};
use crate::errors::CatchledgerError;
use crate::jsonvalue::JsonValue;
use crate::models::RunRecord;

pub const TOKEN_USAGE_FORMAT: &str = "catchledger-token-usage/1";
pub const TOKEN_COST_FORMAT: &str = "catchledger-token-cost/1";

/// Read one token price text the way the price reading of
/// PORT-4 reads the minute price, refusing a text that is not a
/// decimal number and a value that stands below zero or is not
/// finite, in the wording that names the price as it stands.
pub fn read_token_price(text: &str, which: &str) -> Result<DecimalValue, CatchledgerError> {
    let stripped = crate::dates::py_strip(text).to_string();
    let rate = match crate::decimal::parse_decimal(&stripped) {
        Ok(rate) => rate,
        Err(()) => {
            return Err(refusal(&format!(
                "the {which} price per token must be a decimal number"
            )))
        }
    };
    match &rate {
        DecimalValue::NonFinite => Err(refusal(&format!(
            "the {which} price per token must be nonnegative"
        ))),
        DecimalValue::Finite {
            negative, coeff, ..
        } if *negative && !coeff.is_zero() => Err(refusal(&format!(
            "the {which} price per token must be nonnegative"
        ))),
        _ => Ok(rate),
    }
}

/// The entries of one usage document, in the order they stand.
pub struct TokenUsage {
    pub entries: Vec<(String, i64, i64)>,
}

fn refusal(message: &str) -> CatchledgerError {
    CatchledgerError::new(message)
}

/// Read one count of one entry, an exact integer at or past zero.
fn read_count(entry: &JsonValue, field: &str) -> Result<i64, CatchledgerError> {
    match entry.get(field) {
        Some(JsonValue::Int(value)) if *value >= 0 => Ok(*value),
        Some(JsonValue::Int(_)) => Err(refusal(&format!(
            "the {field} of a token usage entry must not be negative"
        ))),
        Some(JsonValue::BigInt(_)) => Err(refusal("the token counts outgrow the ledger")),
        _ => Err(refusal(&format!(
            "the {field} of a token usage entry must be an integer"
        ))),
    }
}

/// Validate one usage document and read its entries.
pub fn read_token_usage(document: &JsonValue) -> Result<TokenUsage, CatchledgerError> {
    if document.get("format").and_then(JsonValue::as_text) != Some(TOKEN_USAGE_FORMAT) {
        return Err(refusal(
            "the token usage document does not carry the catchledger-token-usage/1 format",
        ));
    }
    let Some(runs) = document.get("runs").and_then(JsonValue::as_list) else {
        return Err(refusal("the token usage runs must be a list"));
    };
    let mut entries: Vec<(String, i64, i64)> = Vec::new();
    for entry in runs {
        if entry.as_object().is_none() {
            return Err(refusal("a token usage entry must be an object"));
        }
        let Some(run) = entry.get("run").and_then(JsonValue::as_text) else {
            return Err(refusal("the run of a token usage entry must be text"));
        };
        if entries.iter().any(|(name, _, _)| name == run) {
            return Err(refusal("the token usage document names a run twice"));
        }
        let input_tokens = read_count(entry, "input_tokens")?;
        let output_tokens = read_count(entry, "output_tokens")?;
        entries.push((run.to_string(), input_tokens, output_tokens));
    }
    Ok(TokenUsage { entries })
}

/// Read one integer field of one object standing in a value.
fn int_field(value: Option<&JsonValue>, field: &str) -> i64 {
    match value.and_then(|object| object.get(field)) {
        Some(JsonValue::Int(number)) => *number,
        _ => 0,
    }
}

/// The token figures of one check.
pub struct TokenCheck {
    pub name: String,
    pub fires: i64,
    pub catches: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub token_cost_usd: f64,
}

/// The token figures of one run of the tool.
pub struct TokenFigures {
    pub checks: Vec<TokenCheck>,
    pub fires: i64,
    pub catches: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub token_cost_usd: f64,
}

/// Close a pipeline value at six decimal places, as the ledger
/// closing closes its money values.
fn close_money(value: DecimalValue) -> f64 {
    match quantize_six(&value) {
        Quantized::Value(closed) => closed,
        Quantized::Overflow => quantize_six_full(&value),
    }
}

/// Price one pair of token sums through the decimal pipeline.
fn price_tokens(
    input_tokens: i64,
    output_tokens: i64,
    input_price: &DecimalValue,
    output_price: &DecimalValue,
) -> f64 {
    let input_cost = decimal_mul(&DecimalValue::from_i64(input_tokens), input_price);
    let output_cost = decimal_mul(&DecimalValue::from_i64(output_tokens), output_price);
    close_money(decimal_add(&input_cost, &output_cost))
}

/// Build the token figures from the built ledger, the runs, and
/// the usage document. The fires and catches stand as the ledger
/// states them, and the totals price from the total sums rather
/// than from the closed check values.
pub fn build_token_figures(
    ledger: &JsonValue,
    runs: &[RunRecord],
    usage: &TokenUsage,
    input_price: &DecimalValue,
    output_price: &DecimalValue,
) -> Result<TokenFigures, CatchledgerError> {
    let by_run: HashMap<&str, (i64, i64)> = usage
        .entries
        .iter()
        .map(|(name, input, output)| (name.as_str(), (*input, *output)))
        .collect();
    let empty: Vec<JsonValue> = Vec::new();
    let ledger_checks = ledger
        .get("checks")
        .and_then(JsonValue::as_list)
        .unwrap_or(&empty);
    let mut checks: Vec<TokenCheck> = Vec::new();
    let mut total_input: i128 = 0;
    let mut total_output: i128 = 0;
    let mut total_fires: i64 = 0;
    let mut total_catches: i64 = 0;
    for check in ledger_checks {
        let name = check
            .get("name")
            .and_then(JsonValue::as_text)
            .unwrap_or_default()
            .to_string();
        let scorecard = check.get("scorecard");
        let fires = int_field(scorecard, "fires");
        let catches = int_field(scorecard, "catches");
        let mut input_sum: i128 = 0;
        let mut output_sum: i128 = 0;
        for run in runs.iter().filter(|run| run.check == name) {
            if let Some((input, output)) = by_run.get(run.identifier.as_str()) {
                input_sum += *input as i128;
                output_sum += *output as i128;
            }
        }
        if input_sum > i64::MAX as i128 || output_sum > i64::MAX as i128 {
            return Err(refusal("the token counts outgrow the ledger"));
        }
        let (input_tokens, output_tokens) = (input_sum as i64, output_sum as i64);
        total_input += input_sum;
        total_output += output_sum;
        total_fires += fires;
        total_catches += catches;
        checks.push(TokenCheck {
            name,
            fires,
            catches,
            input_tokens,
            output_tokens,
            token_cost_usd: price_tokens(input_tokens, output_tokens, input_price, output_price),
        });
    }
    if total_input > i64::MAX as i128 || total_output > i64::MAX as i128 {
        return Err(refusal("the token counts outgrow the ledger"));
    }
    Ok(TokenFigures {
        checks,
        fires: total_fires,
        catches: total_catches,
        input_tokens: total_input as i64,
        output_tokens: total_output as i64,
        token_cost_usd: price_tokens(
            total_input as i64,
            total_output as i64,
            input_price,
            output_price,
        ),
    })
}

/// Build the token cost document the figures stand in.
pub fn token_cost_document(
    figures: &TokenFigures,
    repo: &str,
    input_price_text: &str,
    output_price_text: &str,
    token_usage_sha256: &str,
    ledger_sha256: &str,
) -> JsonValue {
    let checks: Vec<JsonValue> = figures
        .checks
        .iter()
        .map(|check| {
            JsonValue::object(vec![
                ("name", JsonValue::Text(check.name.clone())),
                ("fires", JsonValue::Int(check.fires)),
                ("catches", JsonValue::Int(check.catches)),
                ("input_tokens", JsonValue::Int(check.input_tokens)),
                ("output_tokens", JsonValue::Int(check.output_tokens)),
                ("token_cost_usd", JsonValue::Float(check.token_cost_usd)),
            ])
        })
        .collect();
    JsonValue::object(vec![
        ("format", JsonValue::Text(TOKEN_COST_FORMAT.to_string())),
        ("repo", JsonValue::Text(repo.to_string())),
        (
            "pricing",
            JsonValue::object(vec![
                ("currency", JsonValue::Text("USD".to_string())),
                (
                    "input_token_price",
                    JsonValue::Text(input_price_text.to_string()),
                ),
                (
                    "output_token_price",
                    JsonValue::Text(output_price_text.to_string()),
                ),
            ]),
        ),
        (
            "source",
            JsonValue::object(vec![
                (
                    "token_usage_sha256",
                    JsonValue::Text(token_usage_sha256.to_string()),
                ),
                ("ledger_sha256", JsonValue::Text(ledger_sha256.to_string())),
            ]),
        ),
        ("checks", JsonValue::List(checks)),
        (
            "totals",
            JsonValue::object(vec![
                ("fires", JsonValue::Int(figures.fires)),
                ("catches", JsonValue::Int(figures.catches)),
                ("input_tokens", JsonValue::Int(figures.input_tokens)),
                ("output_tokens", JsonValue::Int(figures.output_tokens)),
                ("token_cost_usd", JsonValue::Float(figures.token_cost_usd)),
            ]),
        ),
    ])
}
