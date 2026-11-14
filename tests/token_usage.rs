//! The pins of slice 4, the token usage join.
//!
//! The library pins read the validation and the figures through
//! the library surface, and the door pins drive the binary over
//! the example inputs in scratch directories that carry the
//! process id in their names, as the standing pins do.

use std::path::{Path, PathBuf};
use std::process::Command;

use catchledger::decimal::parse_decimal;
use catchledger::jsonvalue::JsonValue;
use catchledger::models::RunRecord;
use catchledger::reader::read_json_document;
use catchledger::token::TokenUsage;
use catchledger::{build_token_figures, read_token_price, read_token_usage, token_cost_document};

const USAGE_LINE: &str = "usage: catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --output PATH --report PATH --packs-dir DIR [--repo-label TEXT] [--cost-per-minute TEXT] [--token-usage PATH --token-output PATH] [--price-per-input-token TEXT] [--price-per-output-token TEXT]\n";

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
