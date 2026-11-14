//! The recorded command line battery of obligation PORT3-4.
//!
//! The recordings were made by running the landed Python command
//! line as a black box over invocations drawn from its own suite
//! and from the argument edges the contract probe recorded. Each
//! case stands in a scratch directory with its input files, and
//! its steps run there in order. The comparison is at content
//! level, the exit code and the standard error text and the
//! written files byte for byte. For a step the Python argument
//! reader refused, the recording carries no standard error text,
//! because the contract states the argument reader texts stand
//! outside the port, and the step instead asserts the binary
//! writes its own usage line byte for byte, the line standing in
//! the runner below as the pin it is, and nothing to standard
//! output.
//!
//! The clone case is the one case whose recorded bytes are
//! generation bound. The clone digest is taken over the running
//! git log text, whose UTC date spelling changed at git 2.45, so
//! the git history digest field of the recorded ledger and packs
//! can stand only for the git generation that recorded it. The
//! runner rebuilds the clone by the same recipe the generator
//! used, recomputes that digest from the live git text by the
//! PORT2-7 method, requires the digest the binary wrote to equal
//! the recomputation, and compares the written files byte for
//! byte with the recorded digest field normalized to it. The
//! note stands in the recordings file beside the case.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use catchledger::jsonvalue::JsonValue;
use catchledger::{read_json_document, sha256_bytes};

/// The usage line of the shell with its newline, carried here as
/// the pin a usage refusal step byte compares its standard error
/// against, so a step the argument reader refused cannot pass on
/// the tool refusal line standing in its place.
const USAGE_LINE: &str = "usage: catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --output PATH --report PATH --packs-dir DIR [--repo-label TEXT] [--cost-per-minute TEXT] [--token-usage PATH --token-output PATH] [--price-per-input-token TEXT] [--price-per-output-token TEXT]\n";

fn recordings() -> Vec<JsonValue> {
    recordings_at("/tests/conformance/cli_cases.json")
}

fn recordings_at(suffix: &str) -> Vec<JsonValue> {
    let path = format!("{}{suffix}", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(&path).expect("the battery recordings stand in the tree");
    let document =
        read_json_document(&text, "battery recordings").expect("the battery recordings parse");
    document
        .get("cases")
        .and_then(|cases| cases.as_list())
        .expect("the battery recordings carry a case list")
        .to_vec()
}

fn text_of(value: &JsonValue) -> &str {
    value.as_text().unwrap_or("")
}

fn hex_bytes(text: &str) -> Vec<u8> {
    let bytes = text.as_bytes();
    assert!(
        bytes.len().is_multiple_of(2),
        "hex text carries whole bytes"
    );
    (0..bytes.len())
        .step_by(2)
        .map(|at| {
            let pair = std::str::from_utf8(&bytes[at..at + 2]).expect("hex text is text");
            u8::from_str_radix(pair, 16).expect("hex text is hex")
        })
        .collect()
}

fn case_dir(index: usize, name: &str) -> PathBuf {
    let slug: String = name
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect();
    std::env::temp_dir()
        .join(format!("catchledger-cli-battery-{}", std::process::id()))
        .join(format!("case-{index:03}-{slug}"))
}

/// A scratch directory that removes itself when it leaves scope,
/// so a panicking assertion leaves no state behind and concurrent
/// test processes never share a path.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn hold(path: &Path) -> ScratchDir {
        ScratchDir(path.to_path_buf())
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn collect_files(root: &Path, dir: &Path, into: &mut BTreeMap<String, Vec<u8>>) {
    let entries = std::fs::read_dir(dir).expect("a case directory reads");
    for entry in entries {
        let path = entry.expect("a case directory entry reads").path();
        if path.is_dir() {
            collect_files(root, &path, into);
        } else {
            let rel = path
                .strip_prefix(root)
                .expect("a written file stands under the case directory")
                .to_string_lossy()
                .replace('\\', "/");
            let bytes = std::fs::read(&path).expect("a written file reads");
            into.insert(rel, bytes);
        }
    }
}

/// Build the synthetic clone the generator built for the clone
/// case, the synthetic data recipe of the retired Python suite,
/// two commits with pinned names and dates and a rename between
/// them.
fn build_clone(dir: &Path) {
    std::fs::create_dir_all(dir).expect("the clone directory stands");
    let git = |args: &[&str], date: Option<&str>| {
        let mut command = Command::new("git");
        command
            .arg("-C")
            .arg(dir)
            .args(args)
            .env("GIT_AUTHOR_NAME", "Synthetic Dev")
            .env("GIT_AUTHOR_EMAIL", "synthetic@example.invalid")
            .env("GIT_COMMITTER_NAME", "Synthetic Dev")
            .env("GIT_COMMITTER_EMAIL", "synthetic@example.invalid");
        if let Some(date) = date {
            command
                .env("GIT_AUTHOR_DATE", date)
                .env("GIT_COMMITTER_DATE", date);
        }
        let output = command.output().expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} failed with {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-q", "-b", "main", "--object-format=sha1"], None);
    std::fs::create_dir_all(dir.join("src")).expect("the source directory stands");
    std::fs::create_dir_all(dir.join("docs")).expect("the docs directory stands");
    std::fs::write(dir.join("src/code.py"), "value = 1\n").expect("the first source writes");
    std::fs::write(dir.join("docs/guide.md"), "Synthetic guide.\n")
        .expect("the first guide writes");
    git(&["add", "."], None);
    git(
        &["commit", "-q", "-m", "Synthetic first commit"],
        Some("2026-01-01T00:00:00Z"),
    );
    std::fs::write(dir.join("src/code.py"), "value = 2\n").expect("the second source writes");
    git(&["mv", "docs/guide.md", "docs/manual.md"], None);
    git(&["add", "."], None);
    git(
        &["commit", "-q", "-m", "Synthetic second commit"],
        Some("2026-01-01T01:00:00Z"),
    );
}

/// Rerun the clone reading's stated git invocation against the
/// built clone and hash the translated output, the PORT2-7
/// recomputation the clone case digest stands proven by.
fn live_clone_digest(dir: &Path) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["log", "--reverse", "--topo-order", "-M", "--name-status"])
        .arg("--pretty=format:commit %H%nparents %P%ndate %cI%nsubject %s")
        .output()
        .expect("the verification git runs");
    assert!(output.status.success(), "the verification git log reads");
    let text = String::from_utf8(output.stdout).expect("the clone log output is UTF-8");
    let translated = text.replace("\r\n", "\n").replace('\r', "\n");
    sha256_bytes(translated.as_bytes())
}

fn recorded_git_digest(outputs: &BTreeMap<String, Vec<u8>>) -> String {
    let ledger = outputs
        .iter()
        .find(|(rel, _)| rel.ends_with("ledger.json"))
        .map(|(_, bytes)| bytes)
        .expect("the clone case writes a ledger");
    let text = String::from_utf8(ledger.clone()).expect("the ledger is text");
    let document = read_json_document(&text, "recorded ledger").expect("the ledger parses");
    document
        .get("source")
        .and_then(|source| source.get("git_history_sha256"))
        .and_then(|digest| digest.as_text())
        .expect("the ledger carries its git history digest")
        .to_string()
}

fn run_case(index: usize, case: &JsonValue) {
    let name = text_of(case.get("name").expect("each case carries a name")).to_string();
    let dir = case_dir(index, &name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("a stale case directory clears");
    }
    std::fs::create_dir_all(&dir).expect("a case directory stands");
    let _scratch = ScratchDir::hold(&dir);
    let setup = case.get("setup").and_then(|value| value.as_text());
    if setup == Some("clone") {
        build_clone(&dir.join("clone"));
    } else if let Some(mkdir) = setup.and_then(|text| text.strip_prefix("mkdir:")) {
        std::fs::create_dir_all(dir.join(mkdir)).expect("a setup directory stands");
    }
    let inputs = case
        .get("inputs")
        .and_then(|value| value.as_list())
        .expect("each case carries its inputs")
        .clone();
    let mut input_names: Vec<String> = Vec::new();
    for input in &inputs {
        let rel = text_of(input.get("path").expect("an input carries its path"));
        input_names.push(rel.to_string());
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("an input directory stands");
        }
        let bytes = match input.get("hex") {
            Some(hex) => hex_bytes(text_of(hex)),
            None => text_of(input.get("text").expect("an input carries its text"))
                .as_bytes()
                .to_vec(),
        };
        std::fs::write(&path, bytes).expect("an input file is written");
    }
    let dir_text = dir
        .to_str()
        .expect("a case directory path is text")
        .to_string();
    let steps = case
        .get("steps")
        .and_then(|value| value.as_list())
        .expect("each case carries its steps");
    for step in steps {
        let argv: Vec<String> = step
            .get("argv")
            .and_then(|value| value.as_list())
            .expect("each step carries its arguments")
            .iter()
            .map(|part| text_of(part).replace("{DIR}", &dir_text))
            .collect();
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .args(&argv)
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        let expected_exit = match step.get("exit") {
            Some(JsonValue::Int(code)) => *code,
            _ => panic!("each step carries its exit code"),
        };
        assert_eq!(
            output.status.code(),
            Some(expected_exit as i32),
            "case {name} step {argv:?} exits with a different code"
        );
        match step.get("stderr") {
            Some(JsonValue::Text(expected)) => {
                let stderr =
                    String::from_utf8(output.stderr.clone()).expect("standard error is text");
                assert_eq!(
                    stderr, *expected,
                    "case {name} step {argv:?} speaks a different standard error"
                );
            }
            _ => {
                assert_eq!(
                    output.stderr,
                    USAGE_LINE.as_bytes(),
                    "case {name} step {argv:?} writes its own usage line to standard error"
                );
            }
        }
        let stdout_empty = match step.get("stdout_empty") {
            Some(JsonValue::Bool(standing)) => *standing,
            _ => panic!("each step carries its standard output standing"),
        };
        if stdout_empty {
            assert!(
                output.stdout.is_empty(),
                "case {name} step {argv:?} writes to standard output"
            );
        } else {
            assert!(
                !output.stdout.is_empty(),
                "case {name} step {argv:?} answers the help flag on standard output"
            );
        }
    }
    let mut written: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    collect_files(&dir, &dir, &mut written);
    for rel in &input_names {
        written.remove(rel);
    }
    if setup == Some("clone") {
        written.retain(|rel, _| !rel.starts_with("clone/"));
    }
    let mut expected: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    if let Some(outputs) = case.get("outputs").and_then(|value| value.as_list()) {
        for output in outputs {
            let rel = text_of(output.get("path").expect("an output carries its path"));
            let text = text_of(output.get("text").expect("an output carries its text"));
            expected.insert(rel.to_string(), text.as_bytes().to_vec());
        }
    }
    if setup == Some("clone") {
        let recorded_digest = recorded_git_digest(&expected);
        let written_digest = recorded_git_digest(&written);
        let recomputed = live_clone_digest(&dir.join("clone"));
        assert_eq!(
            written_digest, recomputed,
            "case {name} writes the digest of the live git text"
        );
        for bytes in expected.values_mut() {
            let text = String::from_utf8(bytes.clone()).expect("a recorded output is text");
            *bytes = text.replace(&recorded_digest, &written_digest).into_bytes();
        }
    }
    assert_eq!(
        written, expected,
        "case {name} writes different files than the record"
    );
    if name == "the outputs carry no local paths" {
        for bytes in written.values() {
            assert!(
                !bytes
                    .windows(dir_text.len())
                    .any(|window| window == dir_text.as_bytes()),
                "case {name} writes a local path into an output"
            );
        }
    }
    std::fs::remove_dir_all(&dir).expect("a case directory clears");
}

/// One input file of one recorded battery case, standing as the
/// single source of the synthetic documents the named pins below
/// drive the binary over.
fn recorded_input(case_name: &str, input: &str) -> String {
    let cases = recordings();
    let case = cases
        .iter()
        .find(|case| case.get("name").and_then(|value| value.as_text()) == Some(case_name))
        .expect("the named battery case stands in the recordings");
    let inputs = case
        .get("inputs")
        .and_then(|value| value.as_list())
        .expect("the named case carries its inputs");
    inputs
        .iter()
        .find(|entry| entry.get("path").and_then(|value| value.as_text()) == Some(input))
        .and_then(|entry| entry.get("text"))
        .map(|text| text_of(text).to_string())
        .expect("the named input stands in the case")
}

fn pin_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("a stale pin directory clears");
    }
    std::fs::create_dir_all(&dir).expect("a pin directory stands");
    dir
}

#[test]
fn test_slice4_battery_replays() {
    let cases = recordings_at("/tests/conformance/slice4_cases.json");
    assert_eq!(cases.len(), 4, "the slice 4 battery carries its four cases");
    for (index, case) in cases.iter().enumerate() {
        run_case(index + 1000, case);
    }
}

#[test]
fn test_cli_battery_matches_the_python_record() {
    let cases = recordings();
    assert_eq!(cases.len(), 40, "the battery carries 40 cases");
    let sections: Vec<(&str, usize)> = vec![
        ("behaviours", 5),
        ("arguments", 21),
        ("refusals", 12),
        ("write_failure", 2),
    ];
    for (section, count) in &sections {
        let standing = cases
            .iter()
            .filter(|case| case.get("section").and_then(|value| value.as_text()) == Some(section))
            .count();
        assert_eq!(
            standing, *count,
            "the battery section {section} carries its recorded count"
        );
    }
    for (index, case) in cases.iter().enumerate() {
        run_case(index, case);
    }
}

/// The argument bytes pin of obligation PORT3-3. The shell takes
/// its arguments as the operating system hands them over, so a
/// workflow run export whose file name carries a byte that is not
/// UTF-8 reads whole and the run it names builds its outputs.
#[cfg(unix)]
#[test]
fn test_a_file_name_carrying_a_non_utf8_byte_is_read() {
    use std::ffi::OsString;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let dir = pin_dir("catchledger-cli-nonutf8");
    let _scratch = ScratchDir::hold(&dir);
    let runs = recorded_input("deterministic outputs over the examples", "runs.json");
    let log = recorded_input("deterministic outputs over the examples", "gitlog.txt");
    let mut bad_name = dir.as_os_str().as_bytes().to_vec();
    bad_name.extend_from_slice(b"/ru\xffns.json");
    let bad_path = PathBuf::from(OsString::from_vec(bad_name));
    std::fs::write(&bad_path, runs.as_bytes()).expect("the export stands under its byte name");
    let log_path = dir.join("gitlog.txt");
    std::fs::write(&log_path, log.as_bytes()).expect("the history stands");
    let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
        .arg("--ci-history")
        .arg(bad_path.as_os_str())
        .arg("--git-log")
        .arg(&log_path)
        .arg("--output")
        .arg(dir.join("out/ledger.json"))
        .arg("--report")
        .arg(dir.join("out/report.md"))
        .arg("--packs-dir")
        .arg(dir.join("out/packs"))
        .output()
        .expect("the binary runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "an export named with a raw byte builds"
    );
    assert!(output.stderr.is_empty());
    assert!(dir.join("out/ledger.json").exists());
    assert!(dir.join("out/packs/001-style-checks.json").exists());
    std::fs::remove_dir_all(&dir).expect("a pin directory clears");
}

/// The nesting band pin of obligation PORT3-3, stated there as a
/// port limit. A value nested inside the workflow run export at
/// 4500 lists deep in a field the reading does not read completes
/// under the Python, and the port refuses the same document whole
/// at its stated reader bound of 4000, in the reader wording,
/// writing nothing.
#[test]
fn test_a_document_nested_past_the_reader_bound_is_refused_by_the_command_line() {
    let dir = pin_dir("catchledger-cli-nesting");
    let _scratch = ScratchDir::hold(&dir);
    let runs = recorded_input("deterministic outputs over the examples", "runs.json");
    let log = recorded_input("deterministic outputs over the examples", "gitlog.txt");
    let nest = format!("{}{}", "[".repeat(4500), "]".repeat(4500));
    let deep = runs.replacen("{\"repo\":", &format!("{{\"deep\": {nest}, \"repo\":"), 1);
    assert_ne!(deep, runs, "the export object stands in the recording");
    let deep_path = dir.join("deep.json");
    std::fs::write(&deep_path, deep.as_bytes()).expect("the deep document stands");
    let log_path = dir.join("gitlog.txt");
    std::fs::write(&log_path, log.as_bytes()).expect("the history stands");
    let output_path = dir.join("out/ledger.json");
    let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
        .arg("--ci-history")
        .arg(&deep_path)
        .arg("--git-log")
        .arg(&log_path)
        .arg("--output")
        .arg(&output_path)
        .arg("--report")
        .arg(dir.join("out/report.md"))
        .arg("--packs-dir")
        .arg(dir.join("out/packs"))
        .output()
        .expect("the binary runs");
    assert_eq!(
        output.status.code(),
        Some(2),
        "a document nested past the reader bound refuses"
    );
    let stderr = String::from_utf8(output.stderr).expect("standard error is text");
    assert_eq!(
        stderr,
        "catchledger error: the workflow run export is not valid JSON\n"
    );
    assert!(!output_path.exists(), "the refused document writes nothing");
    std::fs::remove_dir_all(&dir).expect("a pin directory clears");
}

/// The repository label pin of the PORT3 closing amendment. The
/// label is the one argument word that flows into the outputs as
/// data, standing in the ledger and the report as it was given,
/// so a label carrying a byte that is not UTF-8 is refused by the
/// shell in its own wording where the Python dies uncaught on
/// such a word, and nothing is written.
#[cfg(unix)]
#[test]
fn test_a_repository_label_carrying_a_non_utf8_byte_is_refused() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = pin_dir("catchledger-cli-label");
    let _scratch = ScratchDir::hold(&dir);
    let runs = recorded_input("deterministic outputs over the examples", "runs.json");
    let log = recorded_input("deterministic outputs over the examples", "gitlog.txt");
    let runs_path = dir.join("runs.json");
    std::fs::write(&runs_path, runs.as_bytes()).expect("the export stands");
    let log_path = dir.join("gitlog.txt");
    std::fs::write(&log_path, log.as_bytes()).expect("the history stands");
    let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
        .arg("--ci-history")
        .arg(&runs_path)
        .arg("--git-log")
        .arg(&log_path)
        .arg("--repo-label")
        .arg(OsString::from_vec(b"rep\xffo".to_vec()))
        .arg("--output")
        .arg(dir.join("out/ledger.json"))
        .arg("--report")
        .arg(dir.join("out/report.md"))
        .arg("--packs-dir")
        .arg(dir.join("out/packs"))
        .output()
        .expect("the binary runs");
    assert_eq!(
        output.status.code(),
        Some(2),
        "a label carrying a raw byte refuses"
    );
    let stderr = String::from_utf8(output.stderr).expect("standard error is text");
    assert_eq!(
        stderr,
        "catchledger error: the repository label is not UTF-8 text\n"
    );
    assert!(
        !dir.join("out").exists(),
        "the refused label writes nothing"
    );
    std::fs::remove_dir_all(&dir).expect("a pin directory clears");
}
