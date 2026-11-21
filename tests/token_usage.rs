//! The pins of slice 4, the token usage join.
//!
//! The library pins read the validation and the figures through
//! the library surface, and the door pins drive the binary over
//! the example inputs in scratch directories that carry the
//! process id in their names, as the standing pins do.

use std::path::{Path, PathBuf};
use std::process::Command;

use catchledger::decimal::{decimal_add, decimal_div, decimal_mul, parse_decimal, DecimalValue};
use catchledger::jsonvalue::JsonValue;
use catchledger::models::RunRecord;
use catchledger::reader::read_json_document;
use catchledger::token::TokenUsage;
use catchledger::{build_token_figures, read_token_price, read_token_usage, token_cost_document};

const USAGE_LINE: &str = "usage: catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --output PATH --report PATH --packs-dir DIR [--repo-label TEXT] [--cost-per-minute TEXT] [--token-usage PATH --token-output PATH] [--price-per-input-token TEXT] [--price-per-output-token TEXT] [--check-token-cost PATH] [--check-report PATH] [--check-packs DIR] [--check-ledger PATH]\n";

const USAGE_DOCUMENT: &str = "{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"102\", \"input_tokens\": 1200, \"output_tokens\": 300}, {\"run\": \"101\", \"input_tokens\": 5000, \"output_tokens\": 0}, {\"run\": \"999\", \"input_tokens\": 777, \"output_tokens\": 777}]}";

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn hold(name: &str) -> ScratchDir {
        let dir = std::env::temp_dir().join(format!("catchledger-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("a stale pin directory clears");
        }
        std::fs::create_dir_all(&dir).expect("a pin directory stands");
        ScratchDir(dir)
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn example(name: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name);
    std::fs::read_to_string(path).expect("the example inputs stand in the tree")
}

/// Stage the example inputs and the usage document in one
/// scratch directory and hand back the directory and its guard.
fn staged(name: &str) -> (ScratchDir, PathBuf) {
    let scratch = ScratchDir::hold(name);
    let dir = scratch.0.clone();
    std::fs::write(dir.join("runs.json"), example("synthetic-runs.json")).expect("runs staged");
    std::fs::write(dir.join("gitlog.txt"), example("synthetic-git-log.txt")).expect("log staged");
    std::fs::write(dir.join("usage.json"), USAGE_DOCUMENT).expect("usage staged");
    (scratch, dir)
}

fn base_args(dir: &Path) -> Vec<String> {
    vec![
        "--ci-history".into(),
        dir.join("runs.json").to_string_lossy().into_owned(),
        "--git-log".into(),
        dir.join("gitlog.txt").to_string_lossy().into_owned(),
        "--output".into(),
        dir.join("ledger.json").to_string_lossy().into_owned(),
        "--report".into(),
        dir.join("report.md").to_string_lossy().into_owned(),
        "--packs-dir".into(),
        dir.join("packs").to_string_lossy().into_owned(),
    ]
}

fn run_binary(args: &[String]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
        .args(args)
        .output()
        .expect("the binary runs");
    (
        output.status.code().expect("the binary exits with a code"),
        String::from_utf8(output.stdout).expect("standard output is text"),
        String::from_utf8(output.stderr).expect("standard error is text"),
    )
}

#[test]
fn test_token_usage_flag_without_output_flag_refuses() {
    let (_scratch, dir) = staged("lone-usage");
    let mut args = base_args(&dir);
    args.push("--token-usage".into());
    args.push(dir.join("usage.json").to_string_lossy().into_owned());
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2);
    assert_eq!(stderr, USAGE_LINE);
    assert!(!dir.join("ledger.json").exists(), "nothing is written");
}

#[test]
fn test_token_output_flag_without_usage_flag_refuses() {
    let (_scratch, dir) = staged("lone-output");
    let mut args = base_args(&dir);
    args.push("--token-output".into());
    args.push(dir.join("tokens.json").to_string_lossy().into_owned());
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2);
    assert_eq!(stderr, USAGE_LINE);
    assert!(!dir.join("ledger.json").exists(), "nothing is written");
}

#[test]
fn test_the_usage_line_carries_the_token_flags() {
    let (_scratch, dir) = staged("usage-line");
    let mut args = base_args(&dir);
    args.push("--token-usage".into());
    args.push(dir.join("usage.json").to_string_lossy().into_owned());
    let (_exit, _stdout, stderr) = run_binary(&args);
    for flag in [
        "--token-usage PATH",
        "--token-output PATH",
        "--price-per-input-token TEXT",
        "--price-per-output-token TEXT",
    ] {
        assert!(stderr.contains(flag), "the usage line carries {flag}");
    }
}

fn refusal_of(
    result: Result<TokenUsage, catchledger::CatchledgerError>,
) -> catchledger::CatchledgerError {
    match result {
        Ok(_) => panic!("the document reads"),
        Err(error) => error,
    }
}

fn usage_from(text: &str) -> Result<TokenUsage, catchledger::CatchledgerError> {
    let document = read_json_document(text, "token usage").expect("the pin document parses");
    read_token_usage(&document)
}

#[test]
fn test_usage_document_with_wrong_format_refuses() {
    let error = refusal_of(usage_from(
        "{\"format\": \"catchledger-token-usage/2\", \"runs\": []}",
    ));
    assert_eq!(
        error.message(),
        "the token usage document does not carry the catchledger-token-usage/1 format"
    );
    let error = refusal_of(usage_from(
        "{\"format\": \"catchledger-token-usage/1\", \"runs\": {}}",
    ));
    assert_eq!(error.message(), "the token usage runs must be a list");
}

#[test]
fn test_usage_entry_naming_a_run_twice_refuses() {
    let error = refusal_of(usage_from("{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"7\", \"input_tokens\": 1, \"output_tokens\": 2}, {\"run\": \"7\", \"input_tokens\": 3, \"output_tokens\": 4}]}"));
    assert_eq!(
        error.message(),
        "the token usage document names a run twice"
    );
}

#[test]
fn test_usage_entry_with_negative_count_refuses() {
    let error = refusal_of(usage_from("{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"7\", \"input_tokens\": -1, \"output_tokens\": 2}]}"));
    assert_eq!(
        error.message(),
        "the input_tokens of a token usage entry must not be negative"
    );
}

#[test]
fn test_usage_document_with_a_repeated_key_refuses_in_the_reader_wording() {
    let (_scratch, dir) = staged("repeated-key");
    std::fs::write(
        dir.join("usage.json"),
        "{\"format\": \"catchledger-token-usage/1\", \"format\": \"catchledger-token-usage/1\", \"runs\": []}",
    )
    .expect("the repeated key document is staged");
    let mut args = base_args(&dir);
    args.push("--token-usage".into());
    args.push(dir.join("usage.json").to_string_lossy().into_owned());
    args.push("--token-output".into());
    args.push(dir.join("tokens.json").to_string_lossy().into_owned());
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2);
    assert_eq!(
        stderr,
        "catchledger error: the token usage carries the key format more than once\n"
    );
}

fn fabricated_ledger() -> JsonValue {
    JsonValue::object(vec![(
        "checks",
        JsonValue::List(vec![
            JsonValue::object(vec![
                ("name", JsonValue::Text("agent checks".to_string())),
                (
                    "scorecard",
                    JsonValue::object(vec![
                        ("fires", JsonValue::Int(2)),
                        ("catches", JsonValue::Int(1)),
                    ]),
                ),
            ]),
            JsonValue::object(vec![
                ("name", JsonValue::Text("unit checks".to_string())),
                (
                    "scorecard",
                    JsonValue::object(vec![
                        ("fires", JsonValue::Int(1)),
                        ("catches", JsonValue::Int(0)),
                    ]),
                ),
            ]),
        ]),
    )])
}

fn fabricated_runs() -> Vec<RunRecord> {
    let run = |identifier: &str, check: &str| RunRecord {
        identifier: identifier.to_string(),
        check: check.to_string(),
        commit: "aaaa".to_string(),
        conclusion: "success".to_string(),
        created_at: 0,
        started_at: 0,
        updated_at: 60_000_000,
    };
    vec![
        run("r1", "agent checks"),
        run("r2", "agent checks"),
        run("r3", "unit checks"),
    ]
}

#[test]
fn test_usage_joins_by_run_identifier() {
    let usage = usage_from("{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"r1\", \"input_tokens\": 100, \"output_tokens\": 10}, {\"run\": \"r2\", \"input_tokens\": 200, \"output_tokens\": 20}]}")
        .expect("the usage document reads");
    let figures = build_token_figures(
        &fabricated_ledger(),
        &fabricated_runs(),
        &usage,
        &parse_decimal("0").expect("zero parses"),
        &parse_decimal("0").expect("zero parses"),
    )
    .expect("the figures build");
    assert_eq!(figures.checks[0].input_tokens, 300);
    assert_eq!(figures.checks[0].output_tokens, 30);
    assert_eq!(figures.checks[1].input_tokens, 0);
    assert_eq!(figures.input_tokens, 300);
    assert_eq!(figures.fires, 3);
}

#[test]
fn test_usage_entry_naming_no_run_passes_over() {
    let usage = usage_from("{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"nobody\", \"input_tokens\": 999, \"output_tokens\": 999}]}")
        .expect("the usage document reads");
    let figures = build_token_figures(
        &fabricated_ledger(),
        &fabricated_runs(),
        &usage,
        &parse_decimal("1").expect("one parses"),
        &parse_decimal("1").expect("one parses"),
    )
    .expect("the figures build");
    assert_eq!(figures.input_tokens, 0);
    assert_eq!(figures.output_tokens, 0);
    assert_eq!(figures.token_cost_usd, 0.0);
}

#[test]
fn test_token_cost_follows_the_decimal_pipeline() {
    let usage = usage_from("{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"r3\", \"input_tokens\": 7, \"output_tokens\": 7}]}")
        .expect("the usage document reads");
    let figures = build_token_figures(
        &fabricated_ledger(),
        &fabricated_runs(),
        &usage,
        &parse_decimal("0.0000005").expect("the price parses"),
        &parse_decimal("0.00000005").expect("the price parses"),
    )
    .expect("the figures build");
    // Seven input tokens at 0.0000005 close half up at six places
    // to 0.000004, and seven output tokens at 0.00000005 close to
    // 0.000000, so the check and the totals carry 0.000004.
    assert_eq!(figures.checks[1].token_cost_usd, 0.000004);
    assert_eq!(figures.token_cost_usd, 0.000004);
}

#[test]
fn test_negative_token_price_refuses() {
    let error = match read_token_price("-1", "input") {
        Ok(_) => panic!("a negative price refuses"),
        Err(error) => error,
    };
    assert_eq!(
        error.message(),
        "the input price per token must be nonnegative"
    );
    let error = match read_token_price("many", "output") {
        Ok(_) => panic!("a non decimal price refuses"),
        Err(error) => error,
    };
    assert_eq!(
        error.message(),
        "the output price per token must be a decimal number"
    );
}

/// Run the full door over the staged examples with the token
/// flags standing and hand back the scratch guard and the dir.
fn full_token_run(name: &str) -> (ScratchDir, PathBuf) {
    let (scratch, dir) = staged(name);
    let mut args = base_args(&dir);
    args.push("--token-usage".into());
    args.push(dir.join("usage.json").to_string_lossy().into_owned());
    args.push("--token-output".into());
    args.push(dir.join("tokens.json").to_string_lossy().into_owned());
    args.push("--price-per-input-token".into());
    args.push("0.000003".into());
    args.push("--price-per-output-token".into());
    args.push("0.000015".into());
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 0, "the full run stands, stderr {stderr}");
    (scratch, dir)
}

#[test]
fn test_token_cost_document_carries_the_per_check_figures() {
    let (_scratch, dir) = full_token_run("document-figures");
    let text = std::fs::read_to_string(dir.join("tokens.json")).expect("the document stands");
    let document = read_json_document(&text, "token cost").expect("the document parses");
    assert_eq!(
        document.get("format").and_then(JsonValue::as_text),
        Some("catchledger-token-cost/1")
    );
    let checks = document
        .get("checks")
        .and_then(JsonValue::as_list)
        .expect("the checks stand");
    let unit = checks
        .iter()
        .find(|check| check.get("name").and_then(JsonValue::as_text) == Some("unit checks"))
        .expect("the unit checks stand");
    assert_eq!(unit.get("input_tokens"), Some(&JsonValue::Int(6200)));
    assert_eq!(unit.get("output_tokens"), Some(&JsonValue::Int(300)));
    assert_eq!(unit.get("token_cost_usd"), Some(&JsonValue::Float(0.0231)));
    let totals = document.get("totals").expect("the totals stand");
    assert_eq!(totals.get("input_tokens"), Some(&JsonValue::Int(6200)));
    assert_eq!(
        totals.get("token_cost_usd"),
        Some(&JsonValue::Float(0.0231))
    );
}

#[test]
fn test_token_cost_document_digests_match_the_files() {
    let (_scratch, dir) = full_token_run("document-digests");
    let text = std::fs::read_to_string(dir.join("tokens.json")).expect("the document stands");
    let document = read_json_document(&text, "token cost").expect("the document parses");
    let source = document.get("source").expect("the source stands");
    let usage_bytes = std::fs::read(dir.join("usage.json")).expect("the usage file stands");
    let ledger_bytes = std::fs::read(dir.join("ledger.json")).expect("the ledger file stands");
    assert_eq!(
        source
            .get("token_usage_sha256")
            .and_then(JsonValue::as_text),
        Some(catchledger::sha256_bytes(&usage_bytes).as_str())
    );
    assert_eq!(
        source.get("ledger_sha256").and_then(JsonValue::as_text),
        Some(catchledger::sha256_bytes(&ledger_bytes).as_str())
    );
}

#[test]
fn test_report_carries_the_token_section_where_usage_stands() {
    let (_scratch, dir) = full_token_run("report-section");
    let report = std::fs::read_to_string(dir.join("report.md")).expect("the report stands");
    assert!(report.contains("## Token costs"), "the section stands");
    assert!(
        report.contains("| unit checks | 6200 | 300 | USD 0.023100 |"),
        "the check line stands"
    );
    assert!(
        report.contains("| Totals | 6200 | 300 | USD 0.023100 |"),
        "the totals line stands"
    );
}

#[test]
fn test_outputs_without_usage_are_byte_identical() {
    let (scratch, dir) = staged("byte-identity");
    let plain_dir = dir.join("plain");
    let token_dir = dir.join("token");
    let mut plain_args = base_args(&dir);
    for (index, value) in plain_args.iter_mut().enumerate() {
        if *value == dir.join("ledger.json").to_string_lossy() {
            *value = plain_dir.join("ledger.json").to_string_lossy().into_owned();
            let _ = index;
        }
        if *value == dir.join("report.md").to_string_lossy() {
            *value = plain_dir.join("report.md").to_string_lossy().into_owned();
        }
        if *value == dir.join("packs").to_string_lossy() {
            *value = plain_dir.join("packs").to_string_lossy().into_owned();
        }
    }
    let (exit, _o, _e) = run_binary(&plain_args);
    assert_eq!(exit, 0);
    let mut token_args = base_args(&dir);
    for value in token_args.iter_mut() {
        if *value == dir.join("ledger.json").to_string_lossy() {
            *value = token_dir.join("ledger.json").to_string_lossy().into_owned();
        }
        if *value == dir.join("report.md").to_string_lossy() {
            *value = token_dir.join("report.md").to_string_lossy().into_owned();
        }
        if *value == dir.join("packs").to_string_lossy() {
            *value = token_dir.join("packs").to_string_lossy().into_owned();
        }
    }
    token_args.push("--token-usage".into());
    token_args.push(dir.join("usage.json").to_string_lossy().into_owned());
    token_args.push("--token-output".into());
    token_args.push(token_dir.join("tokens.json").to_string_lossy().into_owned());
    let (exit, _o, _e) = run_binary(&token_args);
    assert_eq!(exit, 0);
    assert_eq!(
        std::fs::read(plain_dir.join("ledger.json")).expect("the plain ledger stands"),
        std::fs::read(token_dir.join("ledger.json")).expect("the token ledger stands"),
        "the ledger is byte identical in both standings"
    );
    let plain_packs: Vec<u8> = std::fs::read_dir(plain_dir.join("packs"))
        .expect("the plain packs stand")
        .flat_map(|entry| std::fs::read(entry.expect("an entry").path()).expect("a pack reads"))
        .collect();
    let token_packs: Vec<u8> = std::fs::read_dir(token_dir.join("packs"))
        .expect("the token packs stand")
        .flat_map(|entry| std::fs::read(entry.expect("an entry").path()).expect("a pack reads"))
        .collect();
    assert_eq!(plain_packs, token_packs, "the packs are byte identical");
    assert!(token_dir.join("tokens.json").exists());
    drop(scratch);
}

// The document builder stands under its own pin beside the door
// pins, so a change to its field order fails here by name.
#[test]
fn test_token_cost_document_field_order() {
    let usage = usage_from(USAGE_DOCUMENT).expect("the usage document reads");
    let figures = build_token_figures(
        &fabricated_ledger(),
        &fabricated_runs(),
        &usage,
        &parse_decimal("0").expect("zero parses"),
        &parse_decimal("0").expect("zero parses"),
    )
    .expect("the figures build");
    let document = token_cost_document(&figures, "repo", "0", "0", "usage-digest", "ledger-digest");
    let keys: Vec<&str> = document
        .as_object()
        .expect("the document is an object")
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    assert_eq!(
        keys,
        ["format", "repo", "pricing", "source", "checks", "totals"]
    );
}

/// The behavioural pin of slice 21. A token count
/// standing as a big integer past the signed width in
/// one entry of a usage document refuses in the outgrow
/// sentence of the token module, the one sentence of
/// the gathering of slice 21 no recorded answer and no
/// standing pin reaches. The pin passes on the tree
/// slice 21 starts from, the sentence standing correct
/// there at the arm no witness reaches, and it fails
/// the moment that seat its wording parts or falls, so
/// it binds the unwitnessed member of the family.
#[test]
fn test_a_token_count_past_the_signed_width_refuses_in_the_outgrow_sentence() {
    let error = refusal_of(usage_from(
        "{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"7\", \"input_tokens\": 99999999999999999999999999, \"output_tokens\": 2}]}",
    ));
    assert_eq!(error.message(), "the token counts outgrow the ledger");
}

/// The structural pin of slice 25. The context
/// rounding tail stands spelled once in the decimal
/// source, in the one helper context rounding and
/// division read at their tails, and this pin reads
/// that source and counts the spellings. It fails on
/// the tree slice 25 starts from, at its spelling
/// count, the tail standing spelled at the two tails
/// while the gathered name stands at no standing,
/// and it passes on the gathered tree.
#[test]
fn test_the_rounding_tail_stands_spelled_once_in_the_decimal_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("decimal.rs"),
    )
    .expect("the decimal source stands in the tree");
    assert_eq!(
        source.matches("dropped.cmp(&half)").count(),
        1,
        "the rounding tail stands spelled once in the decimal source"
    );
    assert!(
        source.matches("round_digit_text").count() >= 3,
        "the rounding helper stands named at its definition and at the two places that read it"
    );
}

/// The spelling of a decimal result for the pin of
/// slice 25, its coefficient text and its power of
/// ten exponent read through the public surface.
fn rounded_spelling(value: &DecimalValue) -> (String, i64) {
    match value {
        DecimalValue::Finite { coeff, exp10, .. } => (coeff.to_decimal_string(), *exp10),
        DecimalValue::NonFinite => panic!("a finite value stands"),
    }
}

/// The behavioural pin of slice 25. The context
/// rounding of the decimal module rounds exact ties
/// to even under the bare digit test and rounds an
/// exact tie up where a remainder stands beyond it
/// at division, and this pin drives multiplication
/// and division at exact tie values in both sticky
/// postures through the public surface and asserts
/// the rounded coefficient and the exponent at each
/// cell. It passes on the tree slice 25 starts from,
/// the tail standing correct there at both of its
/// spellings, and it fails the moment either spelling
/// parts from the stated rounding, so it binds the
/// tie cells no recorded answer reaches.
#[test]
fn test_decimal_ties_round_as_the_context_states_at_multiplication_and_division() {
    let one = parse_decimal("1").expect("one parses");
    let two = parse_decimal("2").expect("two parses");
    // An exact tie at multiplication on an even kept
    // digit rounds to even, the kept digit standing.
    let product = decimal_mul(
        &parse_decimal("10000000000000000000000000025").expect("the operand parses"),
        &one,
    );
    assert_eq!(
        rounded_spelling(&product),
        ("1000000000000000000000000002".to_string(), 1)
    );
    // An exact tie at multiplication on an odd kept
    // digit rounds up.
    let product = decimal_mul(
        &parse_decimal("10000000000000000000000000035").expect("the operand parses"),
        &one,
    );
    assert_eq!(
        rounded_spelling(&product),
        ("1000000000000000000000000004".to_string(), 1)
    );
    // An exact tie at division with no remainder
    // beyond it rounds to even on an even kept digit,
    // under the bare digit test.
    let quotient = decimal_div(
        &parse_decimal("10000000000000000000000000025").expect("the operand parses"),
        &one,
    );
    assert_eq!(
        rounded_spelling(&quotient),
        ("1000000000000000000000000002".to_string(), 1)
    );
    // An exact tie at division with no remainder
    // beyond it rounds up on an odd kept digit.
    let quotient = decimal_div(
        &parse_decimal("10000000000000000000000000035").expect("the operand parses"),
        &one,
    );
    assert_eq!(
        rounded_spelling(&quotient),
        ("1000000000000000000000000004".to_string(), 1)
    );
    // An exact tie at division with a remainder
    // beyond it rounds up on an even kept digit, the
    // cell the sticky standing alone moves, the bare
    // digit test at this cell rounding to even. The
    // quotient digits stand at
    // 100000000000000000000000000250 with a remainder
    // of one standing beyond them.
    let quotient = decimal_div(
        &parse_decimal("200000000000000000000000000501").expect("the operand parses"),
        &two,
    );
    assert_eq!(
        rounded_spelling(&quotient),
        ("1000000000000000000000000003".to_string(), 2)
    );
}

/// The parting pin of slice 26. Past an adjusted gap
/// of sixty four the larger term its own dropped text
/// stands at the rounding boundary where its
/// coefficient carries twenty nine digits or more,
/// and the smaller term decides that boundary at the
/// tie standings and at the carry and borrow
/// standings. This pin drives addition through the
/// public surface at one cell of each family and at
/// the reproducing tie shape and asserts the
/// coefficient and the exponent exact alignment
/// returns at each cell. It fails on the tree slice
/// 26 starts from at every cell, the gap arm there
/// returning the larger term rounded on its own
/// dropped text alone, and it passes on the repaired
/// tree.
#[test]
fn test_decimal_add_past_the_gap_rounds_with_exact_alignment_at_the_parting_shapes() {
    // The tie family under the same signs, the kept
    // part ending even. The dropped digit stands
    // exactly five and a nonzero smaller term stands
    // beyond it, so exact alignment rounds up where
    // the bare tie reading rounds to even.
    let sum = decimal_add(
        &parse_decimal("10000000000000000000000000025").expect("the operand parses"),
        &parse_decimal("1e-40").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000003".to_string(), 1)
    );
    // The tie family under opposite signs, the kept
    // part ending odd. The subtraction pulls the
    // dropped text below the tie, so exact alignment
    // rounds down where the bare tie reading rounds
    // up.
    let sum = decimal_add(
        &parse_decimal("10000000000000000000000000035").expect("the operand parses"),
        &parse_decimal("-1e-40").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000003".to_string(), 1)
    );
    // The carry family. The dropped text stands at a
    // four followed by a run of nines reaching the
    // smaller term its first digit, and the addition
    // carries through the run and lifts the dropped
    // text past the tie.
    let sum = decimal_add(
        &parse_decimal("1000000000000000000000000002499999999999999999999999999999999999999999999999999999999999999999999999")
            .expect("the operand parses"),
        &parse_decimal("10000000000000000000000000000000000").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000003".to_string(), 72)
    );
    // The borrow family. The dropped text stands at
    // a five followed by a run of zeros with a
    // nonzero tail, and the subtraction borrows
    // through the run and pulls the dropped text
    // below the tie.
    let sum = decimal_add(
        &parse_decimal("1000000000000000000000000002500000000000000000000000000000000000000000000000000000000000000000000001")
            .expect("the operand parses"),
        &parse_decimal("-10000000000000000000000000000000000").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000002".to_string(), 72)
    );
    // The reaching bare tie. The dropped text
    // stands at a five followed by a run of zeros
    // reaching out to the smaller term its first
    // digit, and the smaller term cancels what
    // stands below that head exactly, so the tie
    // arm reads an empty tail and no borrow and
    // the kept part its parity decides alone. The
    // kept part ends even and the sum stands,
    // where the larger term its own dropped text
    // runs past the tie and rounds up.
    let sum = decimal_add(
        &parse_decimal("1000000000000000000000000002500000000000000000000000000000000000010000000000000000000000000000000000")
            .expect("the operand parses"),
        &parse_decimal("-10000000000000000000000000000000000").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000002".to_string(), 72)
    );
}

/// The agreement pin of slice 26. At the standings
/// the repair leaves unmoved, the gaps at and below
/// sixty four where the arm stands silent, the
/// agreeing shapes past the boundary, the carry
/// form, the zero operands, and the multiplication
/// output pairs the pricing seat adds, this pin
/// asserts tonight its values through the public
/// surface. It passes on the tree slice 26 starts
/// from and on the repaired tree, and it fails the
/// moment a standing the slice states as unmoved
/// moves.
#[test]
fn test_decimal_add_past_the_gap_keeps_tonights_values_at_the_agreeing_shapes() {
    // A gap of sixty four, where the arm stands
    // silent and the alignment runs. The tie shape
    // rounds up under the smaller term beyond it.
    let sum = decimal_add(
        &parse_decimal("10000000000000000000000000025").expect("the operand parses"),
        &parse_decimal("1e-36").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000003".to_string(), 1)
    );
    // A gap of sixty three, aligned the same way.
    let sum = decimal_add(
        &parse_decimal("10000000000000000000000000025").expect("the operand parses"),
        &parse_decimal("1e-35").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000003".to_string(), 1)
    );
    // A larger coefficient of twenty eight digits
    // past the boundary, whose sum has no dropped
    // text of its own for the smaller term to
    // decide, standing exactly as it stands.
    let sum = decimal_add(
        &parse_decimal("1000000000000000000000000002").expect("the operand parses"),
        &parse_decimal("1e-40").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000002".to_string(), 0)
    );
    // A short larger term past the boundary standing
    // in its own representation, the value alignment
    // returns in the context rounded representation
    // standing unmoved in the representation the arm
    // returns tonight.
    let sum = decimal_add(
        &parse_decimal("123e5").expect("the operand parses"),
        &parse_decimal("1e-58").expect("the operand parses"),
    );
    assert_eq!(rounded_spelling(&sum), ("123".to_string(), 5));
    // The carry form past the boundary, one followed
    // by twenty eight zeros, its dropped digit
    // standing at zero.
    let sum = decimal_add(
        &parse_decimal("10000000000000000000000000000").expect("the operand parses"),
        &parse_decimal("1e-40").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000000".to_string(), 1)
    );
    // A zero operand on each side, returning the
    // context rounding of the other through the zero
    // arms the slice does not touch.
    let sum = decimal_add(
        &parse_decimal("0").expect("the operand parses"),
        &parse_decimal("10000000000000000000000000025").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000002".to_string(), 1)
    );
    let sum = decimal_add(
        &parse_decimal("10000000000000000000000000025").expect("the operand parses"),
        &parse_decimal("0").expect("the operand parses"),
    );
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000000000000000002".to_string(), 1)
    );
    // Multiplication outputs standing past the
    // boundary, the shape the pricing seat adds. The
    // larger output stands as the sum.
    let first = decimal_mul(
        &parse_decimal("1e40").expect("the operand parses"),
        &parse_decimal("1e40").expect("the operand parses"),
    );
    let second = decimal_mul(
        &parse_decimal("3").expect("the operand parses"),
        &parse_decimal("1").expect("the operand parses"),
    );
    let sum = decimal_add(&first, &second);
    assert_eq!(rounded_spelling(&sum), ("1".to_string(), 80));
    // A twenty eight digit multiplication output
    // standing past the boundary against a unit
    // output, standing exactly as it stands.
    let first = decimal_mul(
        &parse_decimal("1000000000000005").expect("the operand parses"),
        &parse_decimal("1000000000000005").expect("the operand parses"),
    );
    let second = decimal_mul(
        &parse_decimal("1e-40").expect("the operand parses"),
        &parse_decimal("1").expect("the operand parses"),
    );
    let sum = decimal_add(&first, &second);
    assert_eq!(
        rounded_spelling(&sum),
        ("1000000000000010000000000000".to_string(), 3)
    );
}

/// The structural pin of slice 28. The six place
/// closing match stands spelled once in the production
/// tree, in the one shared closing the decimal module
/// carries beside its quantize pair, and this pin reads
/// the three sources the gathering moves and counts the
/// spellings. It fails on the tree slice 28 starts
/// from, at its match count, the match standing spelled
/// at the ledger wrapper and at the token wrapper while
/// the shared name stands at no standing, and it passes
/// on the gathered tree.
#[test]
fn test_the_closing_match_stands_spelled_once_in_the_production_sources() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let decimal = std::fs::read_to_string(root.join("decimal.rs"))
        .expect("the decimal source stands in the tree");
    let ledger = std::fs::read_to_string(root.join("ledger.rs"))
        .expect("the ledger source stands in the tree");
    let token = std::fs::read_to_string(root.join("token.rs"))
        .expect("the token source stands in the tree");
    let fallback = "Quantized::Overflow => quantize_six_full(";
    assert_eq!(
        decimal.matches(fallback).count()
            + ledger.matches(fallback).count()
            + token.matches(fallback).count(),
        1,
        "the closing fallback stands spelled once in the production sources"
    );
    assert!(
        decimal.matches("close_six").count()
            + ledger.matches("close_six").count()
            + token.matches("close_six").count()
            >= 5,
        "the shared closing stands named at its definition and at the four places that read it"
    );
}

/// The behavioural pin of slice 28, standing as the
/// rider the inheritance has carried since the scoping
/// of Round 26. The token seat its overflow standing
/// stands witnessed by no recording and no pin before
/// this round, and this pin drives the token figures
/// over the fabricated ledger and a fabricated usage at
/// a price and a count whose product stands across the
/// closing line, the six place rounded coefficient of
/// the sum passing the twenty eight digit context
/// count, and asserts the token cost stands as the full
/// build the capped step refuses, at the check seat and
/// at the totals seat and in both product shapes, with
/// the same shape one power below the line standing as
/// the capped value. It passes on the tree slice 28
/// starts from, the closing standing correct there at
/// both of its spellings, and on the gathered tree
/// alike, and it fails the moment the closing line or
/// the full build parts at either seat.
#[test]
fn test_token_cost_stands_as_the_full_build_past_the_closing_line() {
    // The input shape across the line. One million
    // input tokens at 1e16 sum to 1e22, whose six
    // place rounded coefficient carries twenty nine
    // digits, so the capped step stands Overflow and
    // the seat returns the full build.
    let usage = usage_from(
        "{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"r1\", \"input_tokens\": 1000000, \"output_tokens\": 0}]}",
    )
    .expect("the usage document reads");
    let figures = build_token_figures(
        &fabricated_ledger(),
        &fabricated_runs(),
        &usage,
        &parse_decimal("10000000000000000").expect("the price parses"),
        &parse_decimal("0").expect("zero parses"),
    )
    .expect("the figures build");
    assert_eq!(figures.checks[0].token_cost_usd, 1e22);
    assert_eq!(figures.token_cost_usd, 1e22);
    // The output shape across the line, the same sum
    // standing in the second product of the seat.
    let usage = usage_from(
        "{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"r1\", \"input_tokens\": 0, \"output_tokens\": 1000000}]}",
    )
    .expect("the usage document reads");
    let figures = build_token_figures(
        &fabricated_ledger(),
        &fabricated_runs(),
        &usage,
        &parse_decimal("0").expect("zero parses"),
        &parse_decimal("10000000000000000").expect("the price parses"),
    )
    .expect("the figures build");
    assert_eq!(figures.checks[0].token_cost_usd, 1e22);
    assert_eq!(figures.token_cost_usd, 1e22);
    // One power below the line the same shape stands
    // as the capped value, its six place rounded
    // coefficient carrying twenty eight digits, so the
    // pin binds the line from both sides.
    let usage = usage_from(
        "{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"r1\", \"input_tokens\": 1000000, \"output_tokens\": 0}]}",
    )
    .expect("the usage document reads");
    let figures = build_token_figures(
        &fabricated_ledger(),
        &fabricated_runs(),
        &usage,
        &parse_decimal("1000000000000000").expect("the price parses"),
        &parse_decimal("0").expect("zero parses"),
    )
    .expect("the figures build");
    assert_eq!(figures.checks[0].token_cost_usd, 1e21);
    assert_eq!(figures.token_cost_usd, 1e21);
}
