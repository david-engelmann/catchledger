//! The pins of slice 10, the pack check, over synthetic
//! documents the suite writes itself. The genuine packs are
//! written by a full run of the binary over the synthetic
//! examples, and the check mode is then driven over the same
//! inputs.

use std::path::{Path, PathBuf};
use std::process::Command;

use catchledger::jsonvalue::parse_json;
use catchledger::jsonvalue::JsonValue;
use catchledger::normalize_canary_document;
use catchledger::read_json_document;
use catchledger::validate_evidence_pack;

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

/// Run the full run once so the genuine packs stand in the
/// scratch under packs, and hand back their file names in
/// sorted order.
fn genuine_packs(dir: &Path) -> Vec<String> {
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
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir.join("packs")).expect("the packs stand") {
        let entry = entry.expect("a pack entry reads");
        names.push(entry.file_name().to_string_lossy().into_owned());
    }
    names.sort();
    names
}

/// Copy the standing packs directory whole into a new
/// directory in the scratch and hand back its name, so a pin
/// can edit the copy while the genuine packs stand.
fn copied_packs(dir: &Path, name: &str) -> String {
    let target = dir.join(name);
    std::fs::create_dir_all(&target).expect("the copy directory stands");
    for entry in std::fs::read_dir(dir.join("packs")).expect("the packs stand") {
        let entry = entry.expect("a pack entry reads");
        std::fs::copy(entry.path(), target.join(entry.file_name())).expect("the pack copies");
    }
    name.to_string()
}

fn check_args(dir: &Path, packs: &str) -> Vec<String> {
    vec![
        "--ci-history".into(),
        at(dir, "runs.json"),
        "--git-log".into(),
        at(dir, "gitlog.txt"),
        "--check-packs".into(),
        at(dir, packs),
    ]
}

#[test]
fn test_a_generated_pack_directory_validates_in_check_mode() {
    let (_scratch, dir) = staged("packs-genuine");
    genuine_packs(&dir);
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "packs"));
    assert_eq!(exit, 0, "the genuine packs validate: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}

#[test]
fn test_a_pack_directory_validates_with_token_usage_standing() {
    let (_scratch, dir) = staged("packs-usage-standing");
    genuine_packs(&dir);
    let mut args = check_args(&dir, "packs");
    args.extend([
        "--token-usage".into(),
        at(&dir, "usage.json"),
        "--price-per-input-token".into(),
        "0.001".into(),
        "--price-per-output-token".into(),
        "0.002".into(),
    ]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 0, "the packs stand the same with usage: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}

#[test]
fn test_a_pack_with_a_tampered_figure_is_refused() {
    let (_scratch, dir) = staged("packs-figure");
    let names = genuine_packs(&dir);
    let copy = copied_packs(&dir, "packs-tampered");
    let path = dir.join(&copy).join(&names[0]);
    let genuine = std::fs::read_to_string(&path).expect("the pack reads");
    let marker = "\"fires\": ";
    let figure_at = genuine.find(marker).expect("the figure stands") + marker.len();
    let mut bytes = genuine.clone().into_bytes();
    bytes[figure_at] = if bytes[figure_at] == b'9' {
        b'8'
    } else {
        bytes[figure_at] + 1
    };
    let tampered = String::from_utf8(bytes).expect("the tamper stays text");
    assert_ne!(tampered, genuine, "the tamper lands");
    std::fs::write(&path, &tampered).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, &copy));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the evidence packs disagree with the ledger"),
        "the refusal carries the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_pack_directory_missing_a_pack_is_refused() {
    let (_scratch, dir) = staged("packs-missing-one");
    let names = genuine_packs(&dir);
    let copy = copied_packs(&dir, "packs-short");
    std::fs::remove_file(dir.join(&copy).join(&names[0])).expect("the pack lifts out");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, &copy));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the evidence packs disagree with the ledger"),
        "a missing pack refuses in the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_pack_directory_carrying_an_extra_pack_is_refused() {
    let (_scratch, dir) = staged("packs-extra");
    let names = genuine_packs(&dir);
    let copy = copied_packs(&dir, "packs-wide");
    std::fs::copy(
        dir.join(&copy).join(&names[0]),
        dir.join(&copy).join("999-extra.json"),
    )
    .expect("the extra pack stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, &copy));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the evidence packs disagree with the ledger"),
        "an extra pack refuses in the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_missing_pack_directory_is_refused_in_check_mode() {
    let (_scratch, dir) = staged("packs-missing-dir");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, "no-packs"));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the evidence packs could not be read"),
        "the refusal carries the reading words: {stderr}"
    );
}

#[test]
fn test_an_unparseable_pack_document_is_refused_in_check_mode() {
    let (_scratch, dir) = staged("packs-unparseable");
    let names = genuine_packs(&dir);
    let copy = copied_packs(&dir, "packs-broken");
    std::fs::write(
        dir.join(&copy).join(&names[0]),
        "this is not a pack document",
    )
    .expect("the broken pack stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, &copy));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the evidence pack is not valid JSON"),
        "the refusal carries the reader words under the label: {stderr}"
    );
}

#[test]
fn test_a_respelled_pack_validates_in_check_mode() {
    let (_scratch, dir) = staged("packs-respelled");
    let names = genuine_packs(&dir);
    let copy = copied_packs(&dir, "packs-respelled");
    let path = dir.join(&copy).join(&names[0]);
    let genuine = std::fs::read_to_string(&path).expect("the pack reads");
    let mut respelled = genuine
        .lines()
        .map(|line| line.trim_start())
        .collect::<Vec<_>>()
        .join("\n");
    respelled.push('\n');
    assert_ne!(respelled, genuine, "the respelling lands");
    std::fs::write(&path, &respelled).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, &copy));
    assert_eq!(exit, 0, "the respelled pack validates: {stderr}");
    assert!(stderr.is_empty(), "the check is silent: {stderr}");
}

#[test]
fn test_check_packs_mode_refuses_the_packs_dir_flag() {
    let (_scratch, dir) = staged("packs-packs-dir-flag");
    genuine_packs(&dir);
    let mut args = check_args(&dir, "packs");
    args.extend(["--packs-dir".into(), at(&dir, "other-packs")]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the mode refuses at the usage exit");
    assert!(
        stderr.contains("usage: catchledger"),
        "the refusal is the usage line: {stderr}"
    );
}

#[test]
fn test_check_packs_mode_refuses_the_output_flag() {
    let (_scratch, dir) = staged("packs-output-flag");
    genuine_packs(&dir);
    let mut args = check_args(&dir, "packs");
    args.extend(["--output".into(), at(&dir, "other.json")]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the mode refuses at the usage exit");
    assert!(
        stderr.contains("usage: catchledger"),
        "the refusal is the usage line: {stderr}"
    );
}

#[test]
fn test_a_pack_directory_checked_against_other_inputs_is_refused() {
    let (_scratch, dir) = staged("packs-other-inputs");
    genuine_packs(&dir);
    let mut args = check_args(&dir, "packs");
    args.extend(["--cost-per-minute".into(), "0.5".into()]);
    let (exit, _stdout, stderr) = run_binary(&args);
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the evidence packs disagree with the ledger"),
        "packs priced from other inputs refuse: {stderr}"
    );
}

#[test]
fn test_a_pack_with_catches_spelled_false_is_refused() {
    let (_scratch, dir) = staged("packs-bool-catches");
    let names = genuine_packs(&dir);
    let copy = copied_packs(&dir, "packs-bool");
    let path = dir.join(&copy).join(&names[0]);
    let genuine = std::fs::read_to_string(&path).expect("the pack reads");
    let scorecard_at = genuine.find("\"scorecard\"").expect("the scorecard stands");
    let marker = "\"catches\": 0";
    let spelled_at = scorecard_at
        + genuine[scorecard_at..]
            .find(marker)
            .expect("the figure stands");
    let spelled = format!(
        "{}\"catches\": false{}",
        &genuine[..spelled_at],
        &genuine[spelled_at + marker.len()..]
    );
    assert_ne!(spelled, genuine, "the spelling lands");
    std::fs::write(&path, &spelled).expect("the copy stands");
    let (exit, _stdout, stderr) = run_binary(&check_args(&dir, &copy));
    assert_eq!(exit, 2, "the check refuses at the standing exit");
    assert!(
        stderr.contains("the evidence packs disagree with the ledger"),
        "a boolean spelled over a count refuses in the disagreement words: {stderr}"
    );
}

#[test]
fn test_a_pack_with_catches_spelled_false_refuses_through_the_library() {
    let (_scratch, dir) = staged("packs-library-bool");
    let names = genuine_packs(&dir);
    let ledger_text = std::fs::read_to_string(dir.join("ledger.json")).expect("the ledger reads");
    let ledger = parse_json(&ledger_text).expect("the ledger parses");
    let copy = copied_packs(&dir, "packs-library-bool-copy");
    let path = dir.join(&copy).join(&names[0]);
    let genuine = std::fs::read_to_string(&path).expect("the pack reads");
    let scorecard_at = genuine.find("\"scorecard\"").expect("the scorecard stands");
    let marker = "\"catches\": 0";
    let spelled_at = scorecard_at
        + genuine[scorecard_at..]
            .find(marker)
            .expect("the figure stands");
    let spelled = format!(
        "{}\"catches\": false{}",
        &genuine[..spelled_at],
        &genuine[spelled_at + marker.len()..]
    );
    assert_ne!(spelled, genuine, "the spelling lands");
    let pack = parse_json(&spelled).expect("the pack parses");
    let error = validate_evidence_pack(&pack, &ledger)
        .expect_err("a boolean spelled pack refuses through the library");
    assert_eq!(
        error.message(),
        "the evidence pack disagrees with its ledger",
        "the refusal carries the pack validation words"
    );
}

#[test]
fn test_two_nan_values_stand_unequal_under_the_model_comparison() {
    let first = parse_json("NaN").expect("the word parses");
    let second = parse_json("NaN").expect("the word parses");
    assert!(
        first != second,
        "two NaN values read across a parse boundary stand unequal"
    );
    let one = parse_json("{\"rate\": NaN}").expect("the document parses");
    let two = parse_json("{\"rate\": NaN}").expect("the document parses");
    assert!(
        one != two,
        "documents carrying NaN stand unequal across a parse boundary"
    );
}
#[test]
fn test_canaryfield_row_requirements_stand_over_every_row_ahead_of_any_match() {
    let lacking = parse_json(
        "{\"gates\": [\"unit checks\", \"style checks\"], \"rows\": [{\"seed\": \"seed-00\", \"name\": \"first synthetic row\", \"defect_class\": \"synthetic class\", \"gates\": {\"style checks\": \"caught\"}}, {\"seed\": \"seed-01\", \"name\": \"second synthetic row\", \"defect_class\": \"synthetic class\", \"gates\": {\"unit checks\": \"caught\"}}]}",
    )
    .expect("the pin document parses");
    let lacking_error = normalize_canary_document(Some(&lacking), "unit checks")
        .expect_err("a row lacking the matched gate result refuses the document whole");
    assert_eq!(
        lacking_error.message(),
        "a canaryfield row lacks the matched gate result",
        "the refusal carries the matched gate result words"
    );
    let seedless = parse_json("{\"gates\": [\"unit checks\"], \"rows\": [{\"gates\": {}}]}")
        .expect("the pin document parses");
    let seedless_error = normalize_canary_document(Some(&seedless), "style checks")
        .expect_err("a row lacking its seed data refuses ahead of any gate match");
    assert_eq!(
        seedless_error.message(),
        "each canaryfield row needs its seed data",
        "the refusal carries the seed data words"
    );
}

#[test]
fn test_the_negative_nan_spelling_refuses_at_every_value_position() {
    for text in ["-NaN", "[-NaN]", "{\"value\": -NaN}"] {
        let error = read_json_document(text, "pin document")
            .expect_err("the negative NaN spelling refuses at every value position");
        assert_eq!(
            error.message(),
            "the pin document is not valid JSON",
            "the refusal carries the reader words under the label"
        );
    }
    let nan = read_json_document("NaN", "pin document").expect("the bare NaN spelling reads");
    assert!(
        matches!(nan, JsonValue::Float(value) if value.is_nan()),
        "the bare NaN spelling reads as a not a number float"
    );
    let infinity =
        read_json_document("Infinity", "pin document").expect("the bare Infinity spelling reads");
    assert_eq!(
        infinity,
        JsonValue::Float(f64::INFINITY),
        "the bare Infinity spelling reads as a positive infinite float"
    );
    let negative_infinity = read_json_document("-Infinity", "pin document")
        .expect("the negative Infinity spelling reads");
    assert_eq!(
        negative_infinity,
        JsonValue::Float(f64::NEG_INFINITY),
        "the negative Infinity spelling reads as a negative infinite float"
    );
    let negative_integer =
        read_json_document("-42", "pin document").expect("a negative integer token reads");
    assert_eq!(
        negative_integer,
        JsonValue::Int(-42),
        "a negative integer token reads as the integer it spells"
    );
}
