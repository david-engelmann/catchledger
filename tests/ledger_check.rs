//! The pins of slice 11, the ledger check, over synthetic
//! documents the suite writes itself. The genuine ledger is
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

/// Run the full run once so the genuine ledger stands in the
/// scratch as ledger.json, and hand back its text.
fn genuine_ledger(dir: &Path) -> String {
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
    ];
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 0, "the run stands: {stderr}");
    std::fs::read_to_string(dir.join("ledger.json")).expect("the ledger reads")
}

fn check_args(dir: &Path, ledger: &str) -> Vec<String> {
    vec![
        "--ci-history".into(),
        at(dir, "runs.json"),
        "--git-log".into(),
        at(dir, "gitlog.txt"),
        "--check-ledger".into(),
        at(dir, ledger),
    ]
}

#[test]
fn test_a_generated_ledger_validates_in_check_mode() {
    let (_scratch, dir) = staged("ledger-genuine");
    genuine_ledger(&dir);
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "ledger.json"));
    assert_eq!(exit, 0, "the genuine ledger validates: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}

#[test]
fn test_a_ledger_with_a_tampered_total_is_refused() {
    let (_scratch, dir) = staged("ledger-total");
    let genuine = genuine_ledger(&dir);
    let totals_at = genuine.find("\"totals\"").expect("the totals stand");
    let marker = "\"fires\": ";
    let figure_at = totals_at
        + genuine[totals_at..]
            .find(marker)
            .expect("the figure stands")
        + marker.len();
    let mut bytes = genuine.clone().into_bytes();
    bytes[figure_at] = if bytes[figure_at] == b'9' {
        b'8'
    } else {
        bytes[figure_at] + 1
    };
    let tampered = String::from_utf8(bytes).expect("the tamper stays text");
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(dir.join("checked.json"), &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "checked.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the ledger disagrees with the run's inputs"),
        "the refusal carries the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_ledger_with_a_tampered_source_digest_is_refused() {
    let (_scratch, dir) = staged("ledger-digest");
    let genuine = genuine_ledger(&dir);
    let marker = "\"ci_history_sha256\": \"";
    let digest_at = genuine.find(marker).expect("the digest stands") + marker.len();
    let mut bytes = genuine.clone().into_bytes();
    bytes[digest_at] = if bytes[digest_at] == b'a' { b'b' } else { b'a' };
    let tampered = String::from_utf8(bytes).expect("the tamper stays text");
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(dir.join("checked.json"), &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "checked.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the ledger disagrees with the run's inputs"),
        "a digest no standing check binds refuses in the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_ledger_with_a_tampered_history_window_is_refused() {
    let (_scratch, dir) = staged("ledger-window");
    let genuine = genuine_ledger(&dir);
    let marker = "\"first_run_started_at\": \"";
    let window_at = genuine.find(marker).expect("the window stands") + marker.len();
    let mut bytes = genuine.clone().into_bytes();
    bytes[window_at] = if bytes[window_at] == b'9' {
        b'8'
    } else {
        bytes[window_at] + 1
    };
    let tampered = String::from_utf8(bytes).expect("the tamper stays text");
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(dir.join("checked.json"), &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "checked.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the ledger disagrees with the run's inputs"),
        "a window no standing check binds refuses in the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_missing_ledger_is_refused_in_check_mode() {
    let (_scratch, dir) = staged("ledger-missing");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "no-ledger.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the ledger could not be read"),
        "the refusal carries the reading words: {stderr}"
    );
}

#[test]
fn test_an_unparseable_ledger_document_is_refused_in_check_mode() {
    let (_scratch, dir) = staged("ledger-unparseable");
    std::fs::write(dir.join("broken.json"), "this is not a ledger document")
        .expect("the broken document stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "broken.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the ledger is not valid JSON"),
        "the refusal carries the reader words under the label: {stderr}"
    );
}

#[test]
fn test_a_ledger_with_a_repeated_key_refuses_in_the_reader_wording() {
    let (_scratch, dir) = staged("ledger-repeated-key");
    let genuine = genuine_ledger(&dir);
    let marker = "\"format\": \"catchledger-ledger/1\"";
    let repeated = genuine.replacen(
        marker,
        "\"format\": \"catchledger-ledger/1\",\n  \"format\": \"catchledger-ledger/1\"",
        1,
    );
    assert_ne!(repeated, genuine, "the repetition lands");
    std::fs::write(dir.join("checked.json"), &repeated).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "checked.json"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the ledger carries the key format more than once"),
        "the refusal carries the reader words under the label: {stderr}"
    );
}

#[test]
fn test_a_respelled_ledger_validates_in_check_mode() {
    let (_scratch, dir) = staged("ledger-respelled");
    let genuine = genuine_ledger(&dir);
    let mut respelled = genuine
        .lines()
        .map(|line| line.trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    respelled.push('\n');
    assert_ne!(respelled, genuine, "the respelling lands");
    std::fs::write(dir.join("checked.json"), &respelled).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "checked.json"));
    assert_eq!(exit, 0, "the respelled ledger validates: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}

#[test]
fn test_check_ledger_mode_refuses_the_output_flag() {
    let (_scratch, dir) = staged("ledger-output-flag");
    genuine_ledger(&dir);
    let mut args = check_args(&dir, "ledger.json");
    args.extend(["--output".into(), at(&dir, "other.json")]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the mode refuses at the usage exit");
    assert!(
        stderr.contains("usage: catchledger"),
        "the refusal is the usage line: {stderr}"
    );
}

#[test]
fn test_check_ledger_mode_refuses_a_second_check_flag() {
    let (_scratch, dir) = staged("ledger-second-check");
    genuine_ledger(&dir);
    let mut args = check_args(&dir, "ledger.json");
    args.extend(["--check-packs".into(), at(&dir, "packs")]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the mode refuses at the usage exit");
    assert!(
        stderr.contains("usage: catchledger"),
        "the refusal is the usage line: {stderr}"
    );
}

#[test]
fn test_a_ledger_checked_against_other_inputs_is_refused() {
    let (_scratch, dir) = staged("ledger-other-inputs");
    genuine_ledger(&dir);
    let mut args = check_args(&dir, "ledger.json");
    args.extend(["--cost-per-minute".into(), "0.5".into()]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the ledger disagrees with the run's inputs"),
        "a ledger priced from other inputs refuses: {stderr}"
    );
}

#[test]
fn test_a_ledger_validates_with_token_usage_standing() {
    let (_scratch, dir) = staged("ledger-usage-standing");
    genuine_ledger(&dir);
    let mut args = check_args(&dir, "ledger.json");
    args.extend([
        "--token-usage".into(),
        at(&dir, "usage.json"),
        "--price-per-input-token".into(),
        "0.001".into(),
        "--price-per-output-token".into(),
        "0.002".into(),
    ]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 0, "the ledger stands the same with usage: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}
