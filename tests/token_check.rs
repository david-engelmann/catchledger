//! The pins of slice 8, the token cost check, over synthetic
//! documents the suite writes itself. The genuine token cost
//! document is written by a full run of the binary over the
//! synthetic examples with the authored usage document, and
//! the check mode is then driven over the same inputs.

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Run the full run once so the genuine token cost document
/// stands in the scratch, and hand back its text.
fn genuine_document(dir: &Path) -> String {
    let args = vec![
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
        "--token-usage".into(),
        at(dir, "usage.json"),
        "--token-output".into(),
        at(dir, "tokens.json"),
        "--price-per-input-token".into(),
        "0.001".into(),
        "--price-per-output-token".into(),
        "0.002".into(),
    ];
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 0, "the run stands: {stderr}");
    std::fs::read_to_string(dir.join("tokens.json")).expect("the token document stands")
}

fn check_args(dir: &Path, document: &str) -> Vec<String> {
    vec![
        "--ci-history".into(),
        at(dir, "runs.json"),
        "--git-log".into(),
        at(dir, "gitlog.txt"),
        "--token-usage".into(),
        at(dir, "usage.json"),
        "--price-per-input-token".into(),
        "0.001".into(),
        "--price-per-output-token".into(),
        "0.002".into(),
        "--check-token-cost".into(),
        at(dir, document),
    ]
}

#[test]
fn test_a_generated_token_cost_document_validates_in_check_mode() {
    let (_scratch, dir) = staged("check-genuine");
    genuine_document(&dir);
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "tokens.json"));
    assert_eq!(exit, 0, "the genuine document validates: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}

#[test]
fn test_a_token_cost_document_with_a_tampered_figure_is_refused() {
    let (_scratch, dir) = staged("check-figure");
    let genuine = genuine_document(&dir);
    let tampered = genuine.replace("\"input_tokens\": 6200", "\"input_tokens\": 6201");
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(dir.join("tokens-tampered.json"), &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "tokens-tampered.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the token cost document disagrees with the ledger and the token usage"),
        "the refusal carries the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_token_cost_document_with_a_tampered_digest_is_refused() {
    let (_scratch, dir) = staged("check-digest");
    let genuine = genuine_document(&dir);
    let marker = "\"ledger_sha256\": \"";
    let at = genuine.find(marker).expect("the digest stands") + marker.len();
    let mut bytes = genuine.into_bytes();
    bytes[at] = if bytes[at] == b'0' { b'1' } else { b'0' };
    std::fs::write(dir.join("tokens-tampered.json"), &bytes).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "tokens-tampered.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the token cost document disagrees with the ledger and the token usage"),
        "a digest standing over no ledger refuses: {stderr}"
    );
}

#[test]
fn test_a_missing_token_cost_document_is_refused_in_check_mode() {
    let (_scratch, dir) = staged("check-missing");
    genuine_document(&dir);
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "no-tokens.json"));
    assert_eq!(exit, 2, "a missing document refuses at the standing exit");
    assert!(
        stderr.contains("the token cost could not be read"),
        "the refusal names the missing document: {stderr}"
    );
}

#[test]
fn test_check_mode_refuses_the_output_flag() {
    let (_scratch, dir) = staged("check-output-flag");
    genuine_document(&dir);
    let mut args = check_args(&dir, "tokens.json");
    args.push("--output".into());
    args.push(at(&dir, "other-ledger.json"));
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "an output flag in check mode is a usage refusal");
    assert_eq!(stderr, USAGE_LINE);
}

#[test]
fn test_check_mode_refuses_the_token_output_flag() {
    let (_scratch, dir) = staged("check-token-output-flag");
    genuine_document(&dir);
    let mut args = check_args(&dir, "tokens.json");
    args.push("--token-output".into());
    args.push(at(&dir, "other-tokens.json"));
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(
        exit, 2,
        "the token output flag in check mode is a usage refusal"
    );
    assert_eq!(stderr, USAGE_LINE);
}

#[test]
fn test_check_mode_without_a_token_usage_document_is_refused() {
    let (_scratch, dir) = staged("check-no-usage");
    genuine_document(&dir);
    let args = vec![
        "--ci-history".into(),
        at(&dir, "runs.json"),
        "--git-log".into(),
        at(&dir, "gitlog.txt"),
        "--check-token-cost".into(),
        at(&dir, "tokens.json"),
    ];
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the usage document stands required in the mode");
    assert_eq!(stderr, USAGE_LINE);
}

#[test]
fn test_a_token_cost_document_with_an_edited_price_is_refused() {
    let (_scratch, dir) = staged("check-price");
    let genuine = genuine_document(&dir);
    let tampered = genuine.replace(
        "\"input_token_price\": \"0.001\"",
        "\"input_token_price\": \"0.002\"",
    );
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(dir.join("tokens-tampered.json"), &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "tokens-tampered.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the token cost document disagrees with the ledger and the token usage"),
        "a price the run did not give refuses: {stderr}"
    );
}

#[test]
fn test_a_token_cost_document_with_catches_spelled_true_is_refused() {
    let (_scratch, dir) = staged("check-bool-catches");
    let genuine = genuine_document(&dir);
    let totals_at = genuine.find("\"totals\"").expect("the totals stand");
    let marker = "\"catches\": ";
    let figure_at = totals_at
        + genuine[totals_at..]
            .find(marker)
            .expect("the figure stands")
        + marker.len();
    assert_eq!(
        &genuine[figure_at..figure_at + 1],
        "1",
        "the totals catches stand at one"
    );
    let spelled = format!("{}true{}", &genuine[..figure_at], &genuine[figure_at + 1..]);
    assert_ne!(spelled, genuine, "the spelling lands");
    std::fs::write(dir.join("tokens-tampered.json"), &spelled).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "tokens-tampered.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the token cost document disagrees with the ledger and the token usage"),
        "a boolean spelled over a count refuses in the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_token_cost_document_with_an_integer_spelled_as_a_float_validates() {
    let (_scratch, dir) = staged("check-float-spelling");
    let genuine = genuine_document(&dir);
    let totals_at = genuine.find("\"totals\"").expect("the totals stand");
    let marker = "\"input_tokens\": ";
    let figure_at = totals_at
        + genuine[totals_at..]
            .find(marker)
            .expect("the figure stands")
        + marker.len();
    assert_eq!(
        &genuine[figure_at..figure_at + 4],
        "6200",
        "the totals input tokens stand at 6200"
    );
    let respelled = format!(
        "{}6200.0{}",
        &genuine[..figure_at],
        &genuine[figure_at + 4..]
    );
    assert_ne!(respelled, genuine, "the respelling lands");
    std::fs::write(dir.join("tokens-tampered.json"), &respelled).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "tokens-tampered.json"));
    assert_eq!(exit, 0, "the respelled document validates: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}
