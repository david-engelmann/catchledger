//! The pins of slice 9, the report check, over synthetic
//! documents the suite writes itself. The genuine report is
//! written by a full run of the binary over the synthetic
//! examples, and the check mode is then driven over the same
//! inputs.

use std::path::{Path, PathBuf};
use std::process::Command;

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
