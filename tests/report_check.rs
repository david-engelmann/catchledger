//! The pins of slice 9, the report check, over synthetic
//! documents the suite writes itself. The genuine report is
//! written by a full run of the binary over the synthetic
//! examples, and the check mode is then driven over the same
//! inputs.

use std::path::{Path, PathBuf};
use std::process::Command;

use catchledger::jsonvalue::JsonValue;
use catchledger::render_markdown;

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

fn staged(name: &str) -> (ScratchDir, PathBuf) {
    let scratch = ScratchDir::hold(name);
    let dir = scratch.0.clone();
    std::fs::write(dir.join("runs.json"), example("synthetic-runs.json")).expect("runs staged");
    std::fs::write(dir.join("gitlog.txt"), example("synthetic-git-log.txt")).expect("log staged");
    std::fs::write(dir.join("usage.json"), USAGE_DOCUMENT).expect("usage staged");
    (scratch, dir)
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

fn at(dir: &Path, name: &str) -> String {
    dir.join(name).to_string_lossy().into_owned()
}

/// Run the full run once, with token figures where asked, so
/// the genuine report stands in the scratch, and hand back
/// its text.
fn genuine_report(dir: &Path, with_tokens: bool) -> String {
    let mut args = vec![
        "--ci-history".into(),
        at(dir, "runs.json"),
        "--git-log".into(),
        at(dir, "gitlog.txt"),
        "--output".into(),
        at(dir, "ledger.json"),
        "--report".into(),
        at(dir, "report.md"),
        "--packs-dir".into(),
        at(dir, "packs"),
    ];
    if with_tokens {
        args.extend([
            "--token-usage".into(),
            at(dir, "usage.json"),
            "--token-output".into(),
            at(dir, "tokens.json"),
            "--price-per-input-token".into(),
            "0.001".into(),
            "--price-per-output-token".into(),
            "0.002".into(),
        ]);
    }
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 0, "the run stands: {stderr}");
    std::fs::read_to_string(dir.join("report.md")).expect("the report stands")
}

fn check_args(dir: &Path, document: &str, with_tokens: bool) -> Vec<String> {
    let mut args = vec![
        "--ci-history".into(),
        at(dir, "runs.json"),
        "--git-log".into(),
        at(dir, "gitlog.txt"),
    ];
    if with_tokens {
        args.extend([
            "--token-usage".into(),
            at(dir, "usage.json"),
            "--price-per-input-token".into(),
            "0.001".into(),
            "--price-per-output-token".into(),
            "0.002".into(),
        ]);
    }
    args.extend(["--check-report".into(), at(dir, document)]);
    args
}

#[test]
fn test_a_generated_report_validates_in_check_mode() {
    let (_scratch, dir) = staged("report-genuine");
    genuine_report(&dir, false);
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report.md", false));
    assert_eq!(exit, 0, "the genuine report validates: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}

/// Slice 14 adds the second assertion below. The standing
/// assertion reads the ledger words as a substring, and the
/// two document words of RW-1 carry those words at their
/// head, so the standing assertion alone cannot fail on a
/// build that speaks the two document words in the plain
/// regime as well. The added assertion is the discriminating
/// half of RW-2 for this pin, and no other line of the pin
/// moves.
#[test]
fn test_a_report_with_a_tampered_figure_is_refused() {
    let (_scratch, dir) = staged("report-figure");
    let genuine = genuine_report(&dir, false);
    let tampered = genuine.replace("| Fires | 6 |", "| Fires | 9 |");
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(dir.join("report-tampered.md"), &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report-tampered.md", false));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger"),
        "the refusal carries the disagreement words: {stderr}"
    );
    assert!(
        !stderr.contains("and the token usage"),
        "the plain regime keeps the standing words: {stderr}"
    );
}

#[test]
fn test_a_report_missing_its_final_newline_is_refused() {
    let (_scratch, dir) = staged("report-newline");
    let genuine = genuine_report(&dir, false);
    let stripped = genuine
        .strip_suffix('\n')
        .expect("the report ends with a newline")
        .to_string();
    std::fs::write(dir.join("report-stripped.md"), &stripped).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report-stripped.md", false));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger"),
        "a respelled report is a different document: {stderr}"
    );
}

#[test]
fn test_a_missing_report_is_refused_in_check_mode() {
    let (_scratch, dir) = staged("report-missing");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "no-report.md", false));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report could not be read"),
        "the refusal carries the reading words: {stderr}"
    );
}

#[test]
fn test_check_report_mode_refuses_the_output_flag() {
    let (_scratch, dir) = staged("report-output-flag");
    genuine_report(&dir, false);
    let mut args = check_args(&dir, "report.md", false);
    args.extend(["--output".into(), at(&dir, "other.json")]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the mode refuses at the usage exit");
    assert!(
        stderr.contains("usage: catchledger"),
        "the refusal is the usage line: {stderr}"
    );
}

#[test]
fn test_check_report_mode_refuses_the_report_flag() {
    let (_scratch, dir) = staged("report-report-flag");
    genuine_report(&dir, false);
    let mut args = check_args(&dir, "report.md", false);
    args.extend(["--report".into(), at(&dir, "other.md")]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the mode refuses at the usage exit");
    assert!(
        stderr.contains("usage: catchledger"),
        "the refusal is the usage line: {stderr}"
    );
}

#[test]
fn test_a_report_with_a_token_section_validates_in_check_mode() {
    let (_scratch, dir) = staged("report-token-section");
    let genuine = genuine_report(&dir, true);
    assert!(
        genuine.contains("## Token costs"),
        "the genuine report carries its token section"
    );
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report.md", true));
    assert_eq!(exit, 0, "the token section report validates: {stderr}");
}

/// This pin asserted the ledger words alone until slice 14
/// amended the refusal sentence of RC-3. Its documents and
/// its exit stand unmoved, and its asserted words move as
/// the named mechanical consequence of that amendment, the
/// whole difference between the two sides of its comparison
/// being the token section the check cannot rebuild without
/// the usage document this pin withholds.
#[test]
fn test_a_report_checked_without_its_token_usage_is_refused() {
    let (_scratch, dir) = staged("report-no-usage");
    genuine_report(&dir, true);
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report.md", false));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger and the token usage"),
        "a report whose section the check cannot rebuild refuses: {stderr}"
    );
}

#[test]
fn test_a_report_checked_against_a_tampered_token_usage_is_refused() {
    let (_scratch, dir) = staged("report-tampered-usage");
    genuine_report(&dir, true);
    let tampered = USAGE_DOCUMENT.replace("\"input_tokens\": 1200", "\"input_tokens\": 1201");
    assert_ne!(tampered, USAGE_DOCUMENT, "the tamper lands");
    std::fs::write(dir.join("usage.json"), &tampered).expect("the usage stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report.md", true));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger and the token usage"),
        "the refusal names the documents its comparison reads: {stderr}"
    );
}

#[test]
fn test_a_report_checked_with_a_changed_token_price_is_refused() {
    let (_scratch, dir) = staged("report-changed-price");
    genuine_report(&dir, true);
    let mut args = check_args(&dir, "report.md", true);
    let price_at = args
        .iter()
        .position(|arg| arg == "--price-per-input-token")
        .expect("the price flag stands in the check arguments");
    args[price_at + 1] = "0.003".into();
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger and the token usage"),
        "the refusal names the documents its comparison reads: {stderr}"
    );
}

#[test]
fn test_a_report_without_a_token_section_checked_with_its_token_usage_is_refused() {
    let (_scratch, dir) = staged("report-gained-section");
    let genuine = genuine_report(&dir, false);
    assert!(
        !genuine.contains("## Token costs"),
        "the genuine report carries no token section"
    );
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report.md", true));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger and the token usage"),
        "the refusal names the documents its comparison reads: {stderr}"
    );
}

/// Stage the examples with the first workflow renamed to
/// Token costs, so the staged ledger carries a check whose
/// name spells the token section heading.
fn staged_named(name: &str) -> (ScratchDir, PathBuf) {
    let scratch = ScratchDir::hold(name);
    let dir = scratch.0.clone();
    let runs = example("synthetic-runs.json").replace("unit checks", "Token costs");
    std::fs::write(dir.join("runs.json"), runs).expect("runs staged");
    std::fs::write(dir.join("gitlog.txt"), example("synthetic-git-log.txt")).expect("log staged");
    std::fs::write(dir.join("usage.json"), USAGE_DOCUMENT).expect("usage staged");
    (scratch, dir)
}

/// The deciding pin of the Round 14 repair. A check whose
/// name spells the token section heading puts that heading
/// line into every rendering of its ledger as a check
/// heading, and RW-2 promises the plain regime the standing
/// words. This pin passes on the contract tree, where the
/// standing code speaks the standing words in the plain
/// regime everywhere, and its discriminating validation
/// stands against the unrepaired build, where the refusal
/// names the token usage and the second assertion fails.
#[test]
fn test_a_tampered_plain_report_for_a_check_named_token_costs_is_refused_in_the_standing_words() {
    let (_scratch, dir) = staged_named("report-named-plain");
    let genuine = genuine_report(&dir, false);
    assert!(
        genuine.contains("## Token costs"),
        "the named check puts its heading into the rendering"
    );
    let tampered = genuine.replace("| Fires | 6 |", "| Fires | 9 |");
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(dir.join("report-tampered.md"), &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report-tampered.md", false));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger"),
        "the refusal carries the disagreement words: {stderr}"
    );
    assert!(
        !stderr.contains("and the token usage"),
        "the plain regime keeps the standing words: {stderr}"
    );
}

#[test]
fn test_a_report_with_a_tampered_token_section_count_is_refused() {
    let (_scratch, dir) = staged("report-section-count");
    let genuine = genuine_report(&dir, true);
    let tampered = genuine.replace(
        "| unit checks | 6200 | 300 |",
        "| unit checks | 6201 | 300 |",
    );
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(dir.join("report-tampered.md"), &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report-tampered.md", false));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger and the token usage"),
        "a section standing in the supplied report refuses in the two document words: {stderr}"
    );
}

#[test]
fn test_a_token_report_for_a_check_named_token_costs_checked_without_its_token_usage_is_refused() {
    let (_scratch, dir) = staged_named("report-named-no-usage");
    genuine_report(&dir, true);
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report.md", false));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger and the token usage"),
        "the genuine section standing behind the colliding name refuses: {stderr}"
    );
}

#[test]
fn test_a_token_report_for_a_check_named_token_costs_checked_against_a_tampered_token_usage_is_refused(
) {
    let (_scratch, dir) = staged_named("report-named-tampered-usage");
    genuine_report(&dir, true);
    let tampered = USAGE_DOCUMENT.replace("\"input_tokens\": 1200", "\"input_tokens\": 1201");
    assert_ne!(tampered, USAGE_DOCUMENT, "the tamper lands");
    std::fs::write(dir.join("usage.json"), &tampered).expect("the usage stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "report.md", true));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the report disagrees with the ledger and the token usage"),
        "the refusal names the documents its comparison reads: {stderr}"
    );
}

/// The structural pin of slice 15. The frame of the token
/// section stands spelled once in the report source, in the
/// one definition the renderer opens the section from and
/// the frame scan reads a supplied report against, and this
/// pin reads that source and counts the spellings. It fails
/// on the tree slice 15 starts from, where each spelled
/// frame line stands twice, once where the section is
/// emitted and once where the scan reads it back, and it
/// passes on the gathered tree.
#[test]
fn test_the_token_section_frame_stands_spelled_once_in_the_report_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("report.rs"),
    )
    .expect("the report source stands in the tree");
    for line in [
        "\"## Token costs\"",
        "\"| Check | Input tokens | Output tokens | Token cost |\"",
        "\"| --- | ---: | ---: | ---: |\"",
    ] {
        assert_eq!(
            source.matches(line).count(),
            1,
            "the frame line {line} stands spelled once in the report source"
        );
    }
    assert!(
        source.matches("TOKEN_SECTION_FRAME").count() >= 3,
        "the frame definition stands named at its definition and at both places that read it"
    );
}

/// The structural pin of slice 16. The not available
/// spelling of the report stands spelled once in the
/// report source, in the one definition the rate helper
/// and the money helper read at their null arms and the
/// two fallback sites inside render_token_section read
/// where a money figure fails to build, and this pin
/// reads that source and counts the spellings. It fails
/// on the tree slice 16 starts from, where the phrase
/// stands spelled four times, at the two helper arms and
/// at the two fallback sites, and it passes on the
/// gathered tree.
#[test]
fn test_the_not_available_spelling_stands_spelled_once_in_the_report_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("report.rs"),
    )
    .expect("the report source stands in the tree");
    assert_eq!(
        source.matches("\"not available\"").count(),
        1,
        "the not available spelling stands spelled once in the report source"
    );
    assert!(
        source.matches("NOT_AVAILABLE_TEXT").count() >= 5,
        "the not available definition stands named at its definition and at the four places that read it"
    );
}

/// The structural pin of slice 19. The separator line of
/// the two column tables of the report stands spelled
/// once in the report source, in the one definition the
/// totals push and the scorecard push and the canaryfield
/// counts push inside render_markdown read, and this pin
/// reads that source and counts the spellings. It fails
/// on the tree slice 19 starts from, where the line
/// stands spelled three times, once under each two
/// column header, and it passes on the gathered tree.
#[test]
fn test_the_two_column_separator_stands_spelled_once_in_the_report_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("report.rs"),
    )
    .expect("the report source stands in the tree");
    assert_eq!(
        source.matches("\"| --- | ---: |\"").count(),
        1,
        "the two column separator stands spelled once in the report source"
    );
    assert!(
        source.matches("TWO_COLUMN_SEPARATOR").count() >= 4,
        "the separator definition stands named at its definition and at the three places that read it"
    );
}

/// The structural pin of slice 20. The special spellings
/// of a float in a report cell stand spelled once in the
/// report source, in the one shared helper the plain cell
/// helper and the six place helper read at their heads,
/// and this pin reads that source and counts the
/// spellings. It fails on the tree slice 20 starts from,
/// where each spelling stands twice, once in each helper,
/// and it passes on the gathered tree.
#[test]
fn test_the_float_special_spellings_stand_spelled_once_in_the_report_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("report.rs"),
    )
    .expect("the report source stands in the tree");
    for spelling in ["\"nan\"", "\"inf\"", "\"-inf\""] {
        assert_eq!(
            source.matches(spelling).count(),
            1,
            "the float special spelling {spelling} stands spelled once in the report source"
        );
    }
    assert!(
        source.matches("special_float_text").count() >= 3,
        "the shared helper stands named at its definition and at the two places that read it"
    );
}

/// Build the crafted ledger the behavioural pin of slice
/// 20 renders, a ledger whose one check carries a catch
/// record of three entries whose started_at fields carry
/// the three special floats, a not a number value and a
/// positive infinite value and a negative infinite value,
/// every other field standing at a plain standing.
fn crafted_ledger_with_special_started_at() -> JsonValue {
    let entry = |started: JsonValue| {
        JsonValue::object(vec![
            ("block_run_id", JsonValue::Text("run-1".to_string())),
            ("blocked_commit", JsonValue::Text("commit-1".to_string())),
            ("started_at", started),
            ("outcome", JsonValue::Text("blocked".to_string())),
            (
                "evidence",
                JsonValue::object(vec![
                    ("fix_commits", JsonValue::List(Vec::new())),
                    ("validating_run_id", JsonValue::Text("run-2".to_string())),
                    ("reason", JsonValue::Text("synthetic reason".to_string())),
                ]),
            ),
        ])
    };
    JsonValue::object(vec![
        ("repo", JsonValue::Text("synthetic/repo".to_string())),
        (
            "totals",
            JsonValue::object(vec![
                ("fires", JsonValue::Int(3)),
                ("blocks", JsonValue::Int(3)),
                ("catches", JsonValue::Int(0)),
                ("false_blocks", JsonValue::Int(0)),
                ("unattributed_blocks", JsonValue::Int(0)),
                ("false_block_rate", JsonValue::Int(0)),
                ("runtime_seconds", JsonValue::Int(0)),
                ("runtime_minutes", JsonValue::Int(0)),
                ("total_cost_usd", JsonValue::Int(0)),
            ]),
        ),
        (
            "checks",
            JsonValue::List(vec![JsonValue::object(vec![
                ("name", JsonValue::Text("unit checks".to_string())),
                (
                    "scorecard",
                    JsonValue::object(vec![
                        ("fires", JsonValue::Int(3)),
                        ("blocks", JsonValue::Int(3)),
                        ("judged_commits", JsonValue::Int(3)),
                        ("flaky_commits", JsonValue::Int(0)),
                        ("flake_rate", JsonValue::Int(0)),
                        ("runtime_seconds", JsonValue::Int(0)),
                        ("runtime_minutes", JsonValue::Int(0)),
                        ("catches", JsonValue::Int(0)),
                        ("false_blocks", JsonValue::Int(0)),
                        ("unattributed_blocks", JsonValue::Int(0)),
                        ("false_block_rate", JsonValue::Int(0)),
                        ("total_cost_usd", JsonValue::Int(0)),
                        ("cost_per_catch_usd", JsonValue::Int(0)),
                    ]),
                ),
                (
                    "catch_record",
                    JsonValue::List(vec![
                        entry(JsonValue::Float(f64::NAN)),
                        entry(JsonValue::Float(f64::INFINITY)),
                        entry(JsonValue::Float(f64::NEG_INFINITY)),
                    ]),
                ),
                ("canaryfield_results", JsonValue::Null),
            ])]),
        ),
    ])
}

/// The behavioural pin of slice 20. A special float
/// carried into a plain cell of the report, at the
/// started_at field of a catch record entry the cell
/// reading serves, renders in the plain spellings the
/// cell helper speaks, and not in the JSON spellings the
/// document rendering speaks at the same seats. The pin
/// passes on the tree slice 20 starts from, the plain
/// arms standing correct there, and it fails the moment
/// the arms part or fall to the JSON spelling, so it
/// binds the half of the float family no recorded answer
/// reaches.
#[test]
fn test_a_catch_record_cell_carrying_a_special_float_renders_the_plain_spellings() {
    let report = render_markdown(&crafted_ledger_with_special_started_at())
        .expect("the crafted ledger renders");
    for row in [
        "| run-1 | commit-1 | nan | blocked | none | run-2 | synthetic reason |",
        "| run-1 | commit-1 | inf | blocked | none | run-2 | synthetic reason |",
        "| run-1 | commit-1 | -inf | blocked | none | run-2 | synthetic reason |",
    ] {
        assert!(
            report.contains(row),
            "the rendered catch record carries the plain row {row}: {report}"
        );
    }
    assert!(
        !report.contains("NaN"),
        "no cell of the report carries the JSON not a number spelling: {report}"
    );
    assert!(
        !report.contains("Infinity"),
        "no cell of the report carries the JSON infinite spelling: {report}"
    );
}

/// A rider pin of Round 20, carrying no obligation text
/// of its own. The reading expressions of the three
/// gatherings of the report stand at their counted seats
/// in the report source, the expression that turns the
/// not available constant to text at the four places that
/// read it and the expression that turns the separator
/// constant to text at the three pushes and the zipped
/// comparison that reads the token section frame back at
/// its one validation seat, and this pin reads that
/// source and counts the expressions, so a reading place
/// that stops reading its constant, or a seat that reads
/// it in another dress, fails here where the name floors
/// of the standing pins cannot see it.
#[test]
fn test_the_report_gathering_reading_expressions_stand_at_their_seats() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("report.rs"),
    )
    .expect("the report source stands in the tree");
    assert_eq!(
        source.matches("NOT_AVAILABLE_TEXT.to_string()").count(),
        4,
        "the not available constant stands read at its four reading places"
    );
    assert_eq!(
        source.matches("TWO_COLUMN_SEPARATOR.to_string()").count(),
        3,
        "the separator constant stands read at its three pushes"
    );
    assert_eq!(
        source.matches(".zip(TOKEN_SECTION_FRAME.iter())").count(),
        1,
        "the frame stands read back at its one validation seat"
    );
}

/// The structural pin of slice 22. The currency word
/// of the pricing, the word USD, stands spelled
/// nowhere in the report source, the money helper
/// reading the word from the one definition in the
/// ledger module through its import as the first
/// argument of its format call, and this pin reads
/// that source and counts the spellings. It fails on
/// the tree slice 22 starts from, where the money
/// format opens with the word spelled inside it, and
/// it passes on the gathered tree.
#[test]
fn test_the_currency_word_stands_spelled_nowhere_in_the_report_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("report.rs"),
    )
    .expect("the report source stands in the tree");
    assert_eq!(
        source.matches("\"USD {}\"").count(),
        0,
        "the money format opening stands spelled nowhere in the report source"
    );
    assert_eq!(
        source.matches("\"USD\"").count(),
        0,
        "the currency word stands spelled nowhere in the report source"
    );
    assert!(
        source.matches("CURRENCY_WORD").count() >= 2,
        "the currency name stands named at its import and at the place that reads it"
    );
}
