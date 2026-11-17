//! Owner discharge diag for catchledger Round 14, slice 14.
//! Probe sets A to C over the scoping's staged documents, run
//! hosted on two branches. On the build branch the report
//! door refusals of set A speak the two document words. On
//! the standing code they speak the ledger words alone, and
//! A3 with A4 between them decide the either side reading of
//! RW-1. B3 and set C stand identical on both branches.
//! This file is evidence only and is never merged.

use std::path::{Path, PathBuf};
use std::process::Command;

const EXPECT_TWO_DOCUMENT: bool = false;

const USAGE_DOCUMENT: &str = "{\"format\": \"catchledger-token-usage/1\", \"runs\": [{\"run\": \"102\", \"input_tokens\": 1200, \"output_tokens\": 300}, {\"run\": \"101\", \"input_tokens\": 5000, \"output_tokens\": 0}, {\"run\": \"999\", \"input_tokens\": 777, \"output_tokens\": 777}]}";

const STAGED_TAMPERED_USAGE: &str = "{\"format\":\"catchledger-token-usage/1\",\"runs\":[{\"input_tokens\":1201,\"output_tokens\":300,\"run\":\"102\"},{\"input_tokens\":5000,\"output_tokens\":0,\"run\":\"101\"},{\"input_tokens\":777,\"output_tokens\":777,\"run\":\"999\"}]}";

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn hold(name: &str) -> ScratchDir {
        let dir =
            std::env::temp_dir().join(format!("catchledger-diag-{name}-{}", std::process::id()));
        if dir.exists() {
            std::fs::remove_dir_all(&dir).expect("a stale diag directory clears");
        }
        std::fs::create_dir_all(&dir).expect("a diag directory stands");
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

fn report_check_args(dir: &Path, document: &str, with_tokens: bool) -> Vec<String> {
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

fn expect_report_refusal(stderr: &str) {
    if EXPECT_TWO_DOCUMENT {
        assert!(
            stderr.contains("the report disagrees with the ledger and the token usage"),
            "the refusal names both documents: {stderr}"
        );
    } else {
        assert!(
            stderr.contains("the report disagrees with the ledger"),
            "the refusal carries the ledger words: {stderr}"
        );
        assert!(
            !stderr.contains("and the token usage"),
            "the standing code speaks the ledger words alone: {stderr}"
        );
    }
}

#[test]
fn diag_a1_the_staged_tampered_usage_at_the_report_door() {
    let (_scratch, dir) = staged("a1");
    genuine_report(&dir, true);
    std::fs::write(dir.join("usage.json"), STAGED_TAMPERED_USAGE).expect("the staged usage stands");
    let (exit, _stdout, stderr) = run_binary(&report_check_args(&dir, "report.md", true));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    expect_report_refusal(&stderr);
}

#[test]
fn diag_a2_a_changed_token_price_at_the_report_door() {
    let (_scratch, dir) = staged("a2");
    genuine_report(&dir, true);
    let mut args = report_check_args(&dir, "report.md", true);
    let price_at = args
        .iter()
        .position(|arg| arg == "--price-per-input-token")
        .expect("the price flag stands in the check arguments");
    args[price_at + 1] = "0.003".into();
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    expect_report_refusal(&stderr);
}

#[test]
fn diag_a3_the_token_report_checked_without_its_usage() {
    let (_scratch, dir) = staged("a3");
    genuine_report(&dir, true);
    let (exit, _stdout, stderr) = run_binary(&report_check_args(&dir, "report.md", false));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    expect_report_refusal(&stderr);
}

#[test]
fn diag_a4_the_plain_report_checked_with_usage_handed() {
    let (_scratch, dir) = staged("a4");
    let genuine = genuine_report(&dir, false);
    assert!(
        !genuine.contains("## Token costs"),
        "the genuine report carries no token section"
    );
    let (exit, _stdout, stderr) = run_binary(&report_check_args(&dir, "report.md", true));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    expect_report_refusal(&stderr);
}

#[test]
fn diag_b3_the_genuine_plain_report_exits_in_silence() {
    let (_scratch, dir) = staged("b3");
    genuine_report(&dir, false);
    let (exit, _stdout, stderr) = run_binary(&report_check_args(&dir, "report.md", false));
    assert_eq!(exit, 0, "the genuine plain report validates: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}

#[test]
fn diag_c1_the_staged_tampered_usage_at_the_token_door() {
    let (_scratch, dir) = staged("c1");
    genuine_report(&dir, true);
    std::fs::write(dir.join("usage.json"), STAGED_TAMPERED_USAGE).expect("the staged usage stands");
    let args = vec![
        "--ci-history".into(),
        at(&dir, "runs.json"),
        "--git-log".into(),
        at(&dir, "gitlog.txt"),
        "--token-usage".into(),
        at(&dir, "usage.json"),
        "--price-per-input-token".into(),
        "0.001".into(),
        "--price-per-output-token".into(),
        "0.002".into(),
        "--check-token-cost".into(),
        at(&dir, "tokens.json"),
    ];
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the token cost document disagrees with the ledger and the token usage"),
        "the sibling wording stands unmoved: {stderr}"
    );
}

#[test]
fn diag_c2_the_ledger_door_under_a_changed_repository_label() {
    let (_scratch, dir) = staged("c2");
    let run_args = vec![
        "--ci-history".into(),
        at(&dir, "runs.json"),
        "--git-log".into(),
        at(&dir, "gitlog.txt"),
        "--output".into(),
        at(&dir, "ledger.json"),
        "--report".into(),
        at(&dir, "report.md"),
        "--packs-dir".into(),
        at(&dir, "packs"),
        "--repo-label".into(),
        "alpha".into(),
    ];
    let (exit, _stdout, stderr) = run_binary(&run_args);
    assert_eq!(exit, 0, "the run stands: {stderr}");
    let check_args = vec![
        "--ci-history".into(),
        at(&dir, "runs.json"),
        "--git-log".into(),
        at(&dir, "gitlog.txt"),
        "--repo-label".into(),
        "beta".into(),
        "--check-ledger".into(),
        at(&dir, "ledger.json"),
    ];
    let (exit, _stdout, stderr) = run_binary(&check_args);
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the ledger disagrees with the run's inputs"),
        "the sibling wording stands unmoved: {stderr}"
    );
}
