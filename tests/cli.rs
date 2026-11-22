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
const USAGE_LINE: &str = "usage: catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --output PATH --report PATH --packs-dir DIR [--repo-label TEXT] [--cost-per-minute TEXT] [--token-usage PATH --token-output PATH] [--price-per-input-token TEXT] [--price-per-output-token TEXT] [--check-token-cost PATH] [--check-report PATH] [--check-packs DIR] [--check-ledger PATH]\n";

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

/// The structural pin of slice 17. The default price of
/// the tool stands spelled once in the production
/// source, in the definition the ledger module carries,
/// and the fallback of the command line door reads that
/// definition where it builds its price text, the
/// definition its name standing in the ledger import of
/// the main module and at the fallback reading, and
/// this pin reads the production source whole and
/// counts the spellings. It fails on the tree slice 17
/// starts from, where the value stands spelled twice,
/// at the definition and at the fallback literal, and
/// the definition stands named nowhere in the main
/// module, and it passes on the gathered tree.
#[test]
fn test_the_default_price_stands_spelled_once_in_the_production_source() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let entries = std::fs::read_dir(&source_dir).expect("the production source stands in the tree");
    let mut spellings = 0;
    for entry in entries {
        let path = entry.expect("a production source entry reads").path();
        if path.to_string_lossy().ends_with(".rs") {
            let text = std::fs::read_to_string(&path).expect("a production source file reads");
            spellings += text.matches("\"0.008\"").count();
        }
    }
    assert_eq!(
        spellings, 1,
        "the default price stands spelled once in the production source"
    );
    let main_source = std::fs::read_to_string(source_dir.join("main.rs"))
        .expect("the main source stands in the tree");
    assert_eq!(
        main_source.matches("DEFAULT_COST_PER_MINUTE").count(),
        2,
        "the default price definition stands named in its import and at its fallback"
    );
}

/// The structural pin of slice 18. The two refusal
/// wordings of the reader, the wording that a document
/// could not be read and the wording that a document
/// is not valid JSON, each stand spelled once in the
/// production source, inside the wording functions of
/// the reader module, and the reader reads them at its
/// refusal helper and at the two arms of its file
/// reading and the command line door reads them at the
/// two arms of its read document helper, the names
/// standing in the reader import of the main module
/// and at the reading site of each there, and this pin
/// reads the production source whole and counts the
/// spellings and the standings of the names in the main
/// source. It fails on the tree slice 18 starts from,
/// where the not valid wording stands spelled three
/// times, at the refusal helper and at the file reading
/// of the reader and at the read document helper of the
/// door, and the read wording stands spelled twice, at
/// the file reading and at the door, and the wording
/// functions stand named nowhere in the main module,
/// and it passes on the gathered tree.
#[test]
fn test_the_reader_refusal_wordings_stand_spelled_once_in_the_production_source() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let entries = std::fs::read_dir(&source_dir).expect("the production source stands in the tree");
    let mut not_valid_spellings = 0;
    let mut read_spellings = 0;
    for entry in entries {
        let path = entry.expect("a production source entry reads").path();
        if path.to_string_lossy().ends_with(".rs") {
            let text = std::fs::read_to_string(&path).expect("a production source file reads");
            not_valid_spellings += text.matches("\"the {label} is not valid JSON\"").count();
            read_spellings += text.matches("\"the {label} could not be read\"").count();
        }
    }
    assert_eq!(
        not_valid_spellings, 1,
        "the not valid JSON wording stands spelled once in the production source"
    );
    assert_eq!(
        read_spellings, 1,
        "the could not be read wording stands spelled once in the production source"
    );
    let main_source = std::fs::read_to_string(source_dir.join("main.rs"))
        .expect("the main source stands in the tree");
    assert_eq!(
        main_source.matches("could_not_be_read_wording").count(),
        2,
        "the could not be read wording function stands named in its import and at its reading site"
    );
    assert_eq!(
        main_source.matches("not_valid_json_wording").count(),
        2,
        "the not valid JSON wording function stands named in its import and at its reading site"
    );
}

/// A rider pin of Round 20, carrying no obligation text
/// of its own. The fallback of the command line door
/// reads the default price definition in the one
/// expression that turns it to text, standing once in
/// the main source, and the wording functions of the
/// reader stand at their seats inside the reader source,
/// the could not be read wording at its definition and
/// at the file reading arm and the not valid wording at
/// its definition and at the refusal helper and at the
/// file reading arm, and this pin reads those sources
/// and counts the standings, so a reading place that
/// stops reading its definition, or a seat that reads it
/// in another dress, fails here where the name counts of
/// the standing pins cannot see it.
#[test]
fn test_the_door_and_reader_reading_expressions_stand_at_their_seats() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let main_source = std::fs::read_to_string(source_dir.join("main.rs"))
        .expect("the main source stands in the tree");
    assert_eq!(
        main_source
            .matches("DEFAULT_COST_PER_MINUTE.to_string()")
            .count(),
        1,
        "the default price definition stands read at its one fallback"
    );
    let reader_source = std::fs::read_to_string(source_dir.join("reader.rs"))
        .expect("the reader source stands in the tree");
    assert_eq!(
        reader_source.matches("could_not_be_read_wording").count(),
        2,
        "the could not be read wording stands at its definition and at its file reading seat"
    );
    assert_eq!(
        reader_source.matches("not_valid_json_wording").count(),
        3,
        "the not valid JSON wording stands at its definition and at its two reading seats"
    );
}

/// The structural pin of slice 21. The change line
/// sentence of the ingest module, the words a Git
/// history change line is malformed, stands spelled
/// once in the ingest source, in the one definition the
/// malformed closure of the path decoding reads and the
/// three fresh spellings of the log reading read, and
/// this pin reads that source and counts the spellings.
/// It fails on the tree slice 21 starts from, where the
/// sentence stands spelled four times, and it passes on
/// the gathered tree.
#[test]
fn test_the_change_line_sentence_stands_spelled_once_in_the_ingest_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ingest.rs"),
    )
    .expect("the ingest source stands in the tree");
    assert_eq!(
        source
            .matches("\"a Git history change line is malformed\"")
            .count(),
        1,
        "the change line sentence stands spelled once in the ingest source"
    );
    assert!(
        source.matches("CHANGE_LINE_MALFORMED").count() >= 5,
        "the change line definition stands named at its definition and at the four places that read it"
    );
}

/// The structural pin of slice 21. The export shape
/// sentence of the ingest module, the words the
/// workflow run export must be a list or an object with
/// workflow runs, stands spelled once in the ingest
/// source, in the one definition the two arms of the
/// export reading read, and this pin reads that source
/// and counts the spellings. It fails on the tree slice
/// 21 starts from, where the sentence stands spelled
/// twice, and it passes on the gathered tree.
#[test]
fn test_the_export_shape_sentence_stands_spelled_once_in_the_ingest_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ingest.rs"),
    )
    .expect("the ingest source stands in the tree");
    assert_eq!(
        source
            .matches("\"the workflow run export must be a list or an object with workflow runs\"")
            .count(),
        1,
        "the export shape sentence stands spelled once in the ingest source"
    );
    assert!(
        source.matches("WORKFLOW_RUN_EXPORT_SHAPE").count() >= 3,
        "the export shape definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 21. The identifier
/// sentence of the ingest module, the words a completed
/// workflow run needs an identifier, stands spelled
/// once in the ingest source, in the one definition the
/// two arms of the export reading read, and this pin
/// reads that source and counts the spellings. It fails
/// on the tree slice 21 starts from, where the sentence
/// stands spelled twice, and it passes on the gathered
/// tree.
#[test]
fn test_the_completed_run_identifier_sentence_stands_spelled_once_in_the_ingest_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ingest.rs"),
    )
    .expect("the ingest source stands in the tree");
    assert_eq!(
        source
            .matches("\"a completed workflow run needs an identifier\"")
            .count(),
        1,
        "the identifier sentence stands spelled once in the ingest source"
    );
    assert!(
        source.matches("COMPLETED_RUN_NEEDS_IDENTIFIER").count() >= 3,
        "the identifier definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 21. The local Git
/// sentence of the ingest module, the words the local
/// Git repository could not be read, stands spelled
/// once in the ingest source, in the one definition the
/// two arms of the clone reading read, and this pin
/// reads that source and counts the spellings. It fails
/// on the tree slice 21 starts from, where the sentence
/// stands spelled twice, and it passes on the gathered
/// tree.
#[test]
fn test_the_local_git_sentence_stands_spelled_once_in_the_ingest_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ingest.rs"),
    )
    .expect("the ingest source stands in the tree");
    assert_eq!(
        source
            .matches("\"the local Git repository could not be read\"")
            .count(),
        1,
        "the local Git sentence stands spelled once in the ingest source"
    );
    assert!(
        source.matches("LOCAL_GIT_REPOSITORY_READ").count() >= 3,
        "the local Git definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 21. The outgrow sentence
/// of the token module, the words the token counts
/// outgrow the ledger, stands spelled once in the token
/// source, in the one definition the big integer arm of
/// the count reading reads through the refusal helper
/// and the two width arms of the figures building read
/// through the same helper, and this pin reads that
/// source and counts the spellings. It fails on the
/// tree slice 21 starts from, where the sentence stands
/// spelled three times, and it passes on the gathered
/// tree.
#[test]
fn test_the_token_counts_outgrow_sentence_stands_spelled_once_in_the_token_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("token.rs"),
    )
    .expect("the token source stands in the tree");
    assert_eq!(
        source
            .matches("\"the token counts outgrow the ledger\"")
            .count(),
        1,
        "the outgrow sentence stands spelled once in the token source"
    );
    assert!(
        source.matches("TOKEN_COUNTS_OUTGROW").count() >= 4,
        "the outgrow definition stands named at its definition and at the three places that read it"
    );
}

/// The structural pin of slice 21. The price template
/// of the token module, the words the price per token
/// must be nonnegative under the price named by the
/// caller, stands spelled once in the token source, in
/// the one wording function the two arms of the price
/// reading read, and this pin reads that source and
/// counts the spellings. It fails on the tree slice 21
/// starts from, where the template stands spelled
/// twice, once in each arm, and it passes on the
/// gathered tree.
#[test]
fn test_the_token_price_template_stands_spelled_once_in_the_token_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("token.rs"),
    )
    .expect("the token source stands in the tree");
    assert_eq!(
        source
            .matches("\"the {which} price per token must be nonnegative\"")
            .count(),
        1,
        "the price template stands spelled once in the token source"
    );
    assert!(
        source.matches("nonnegative_price_wording").count() >= 3,
        "the price wording function stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 21. The nonnegative
/// sentence of the ledger module, the words the price
/// per minute must be nonnegative, stands spelled once
/// in the ledger source, in the one definition the two
/// arms of the pricing function read, and this pin
/// reads that source and counts the spellings. It fails
/// on the tree slice 21 starts from, where the sentence
/// stands spelled twice, and it passes on the gathered
/// tree.
#[test]
fn test_the_price_per_minute_sentence_stands_spelled_once_in_the_ledger_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ledger.rs"),
    )
    .expect("the ledger source stands in the tree");
    assert_eq!(
        source
            .matches("\"the price per minute must be nonnegative\"")
            .count(),
        1,
        "the nonnegative sentence stands spelled once in the ledger source"
    );
    assert!(
        source.matches("NONNEGATIVE_PRICE_WORDING").count() >= 3,
        "the nonnegative definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 21. The disagreement
/// sentence of the ledger module, the words the
/// evidence packs disagree with the ledger, stands
/// spelled once in the ledger source, in the one
/// definition the two arms of the pack validation read,
/// and this pin reads that source and counts the
/// spellings. It fails on the tree slice 21 starts from,
/// where the sentence stands spelled twice, and it
/// passes on the gathered tree.
#[test]
fn test_the_packs_disagreement_sentence_stands_spelled_once_in_the_ledger_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ledger.rs"),
    )
    .expect("the ledger source stands in the tree");
    assert_eq!(
        source
            .matches("\"the evidence packs disagree with the ledger\"")
            .count(),
        1,
        "the disagreement sentence stands spelled once in the ledger source"
    );
    assert!(
        source.matches("PACKS_DISAGREEMENT_WORDING").count() >= 3,
        "the disagreement definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 21. The packs read
/// sentence of the main module, the words the evidence
/// packs could not be read, stands spelled once in the
/// main source, in the one definition the two map error
/// seats of the check packs branch of the door read on
/// the reading expression form, and this pin reads that
/// source and counts the spellings. It fails on the
/// tree slice 21 starts from, where the sentence stands
/// spelled twice, and it passes on the gathered tree.
#[test]
fn test_the_evidence_packs_read_sentence_stands_spelled_once_in_the_main_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("main.rs"),
    )
    .expect("the main source stands in the tree");
    assert_eq!(
        source
            .matches("\"the evidence packs could not be read\"")
            .count(),
        1,
        "the packs read sentence stands spelled once in the main source"
    );
    assert!(
        source.matches("EVIDENCE_PACKS_READ").count() >= 3,
        "the packs read definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 22. The currency word
/// of the pricing, the word USD, stands spelled once
/// in the ledger source, in the one definition the
/// pricing function reads at the currency field of the
/// pricing record it builds, and this pin reads that
/// source and counts the spellings. It fails on the
/// tree slice 22 starts from, at its name assertion,
/// the word already standing spelled once in this
/// source at the pricing seat itself while the
/// gathered name stands at no standing, and it passes
/// on the gathered tree.
#[test]
fn test_the_currency_word_stands_spelled_once_in_the_ledger_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ledger.rs"),
    )
    .expect("the ledger source stands in the tree");
    assert_eq!(
        source.matches("\"USD\"").count(),
        1,
        "the currency word stands spelled once in the ledger source"
    );
    assert!(
        source.matches("CURRENCY_WORD").count() >= 2,
        "the currency definition stands named at its definition and at the place that reads it"
    );
}

/// The structural pin of slice 22. The currency word
/// of the pricing, the word USD, stands spelled
/// nowhere in the token source, the pricing object of
/// the token cost document reading the word from the
/// one definition in the ledger module through its
/// import, and this pin reads that source and counts
/// the spellings. It fails on the tree slice 22 starts
/// from, where the word stands spelled once at the
/// currency field of the pricing object, and it passes
/// on the gathered tree.
#[test]
fn test_the_currency_word_stands_spelled_nowhere_in_the_token_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("token.rs"),
    )
    .expect("the token source stands in the tree");
    assert_eq!(
        source.matches("\"USD\"").count(),
        0,
        "the currency word stands spelled nowhere in the token source"
    );
    assert!(
        source.matches("CURRENCY_WORD").count() >= 2,
        "the currency name stands named at its import and at the place that reads it"
    );
}

/// The structural pin of slice 23. The check message,
/// the words the check mode stands on a usage
/// document, stands spelled once in the main source,
/// in the one definition the check token cost branch
/// of the door reads at its two expect seats, and
/// this pin reads that source and counts the
/// spellings. It fails on the tree slice 23 starts
/// from, at its spelling count, the message standing
/// spelled twice at the two seats while the gathered
/// name stands at no standing, and it passes on the
/// gathered tree.
#[test]
fn test_the_check_message_stands_spelled_once_in_the_main_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("main.rs"),
    )
    .expect("the main source stands in the tree");
    assert_eq!(
        source
            .matches("\"the check mode stands on a usage document\"")
            .count(),
        1,
        "the check message stands spelled once in the main source"
    );
    assert!(
        source.matches("CHECK_USAGE_MESSAGE").count() >= 3,
        "the check message definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 23. The scientific
/// message, the words scientific spelling always
/// carries an exponent, stands spelled once in the
/// render source, in the one definition the float
/// spelling function reads at its two expect seats,
/// and this pin reads that source and counts the
/// spellings, the first pin of the suite to read
/// that source. It fails on the tree slice 23 starts
/// from, at its spelling count, the message standing
/// spelled twice at the two seats while the gathered
/// name stands at no standing, and it passes on the
/// gathered tree.
#[test]
fn test_the_scientific_message_stands_spelled_once_in_the_render_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("render.rs"),
    )
    .expect("the render source stands in the tree");
    assert_eq!(
        source
            .matches("\"scientific spelling always carries an exponent\"")
            .count(),
        1,
        "the scientific message stands spelled once in the render source"
    );
    assert!(
        source.matches("EXPONENT_MARKER_MESSAGE").count() >= 3,
        "the scientific message definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 24. The runs label,
/// the one word runs, stands spelled once in the
/// ledger source, in the one definition the history
/// window of build_ledger reads at its two expect
/// seats, and this pin reads that source and counts
/// the spellings. It fails on the tree slice 24
/// starts from, at its spelling count, the label
/// standing spelled twice at the two seats while
/// the gathered name stands at no standing, and it
/// passes on the gathered tree.
#[test]
fn test_the_runs_label_stands_spelled_once_in_the_ledger_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ledger.rs"),
    )
    .expect("the ledger source stands in the tree");
    assert_eq!(
        source.matches("\"runs\"").count(),
        1,
        "the runs label stands spelled once in the ledger source"
    );
    assert!(
        source.matches("RUNS_LABEL").count() >= 3,
        "the runs label definition stands named at its definition and at the two places that read it"
    );
}

/// The structural pin of slice 32. The three outcome
/// counts of the ledger builder, the catches and the
/// false blocks and the unattributed, each stand as a
/// filter and count over the catch record parting only
/// in the outcome word, and each reads the outcome of
/// its item by the one reading spelling of the ledger
/// source. This pin reads the ledger source and counts
/// that spelling, which stands at the three counts and
/// at no other seat of the file. It fails on the day a
/// fourth outcome count stands added in the idiom or
/// one of the three counts stands respelled into a
/// helper, the silent drift the slice states as this
/// family its live risk, and it passes on the tree as
/// landed, the statement of the slice moving no code.
#[test]
fn test_the_outcome_filter_counts_stand_at_three_standings_in_the_ledger_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("ledger.rs"),
    )
    .expect("the ledger source stands in the tree");
    assert_eq!(
        source.matches("item.get(\"outcome\")").count(),
        3,
        "the outcome reading stands at the three outcome filter counts in the ledger source"
    );
}

/// The structural pin of slice 32. The two render
/// writers spell the six scalar arms of the value
/// reading identically, the null arm and the boolean
/// arm and the integer arm and the big integer arm and
/// the float arm and the text arm, the writers parting
/// only in their container arms on layout alone. This
/// pin reads the render source and counts each scalar
/// arm line and the object key sort step line, each
/// standing in both writers and at no third seat. It
/// fails on the day one writer its scalar arm or its
/// sort step line stands respelled apart from its pair, the
/// silent parting the slice states as this family its
/// live risk, and it passes on the tree as landed, the
/// statement of the slice moving no code.
#[test]
fn test_the_render_scalar_arms_stand_spelled_twice_in_the_render_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("render.rs"),
    )
    .expect("the render source stands in the tree");
    assert_eq!(
        source
            .matches("JsonValue::Null => out.push_str(\"null\"),")
            .count(),
        2,
        "the null arm of the render writers stands spelled in both writers"
    );
    assert_eq!(
        source
            .matches(
                "JsonValue::Bool(flag) => out.push_str(if *flag { \"true\" } else { \"false\" }),"
            )
            .count(),
        2,
        "the boolean arm of the render writers stands spelled in both writers"
    );
    assert_eq!(
        source
            .matches("JsonValue::Int(integer) => out.push_str(&integer.to_string()),")
            .count(),
        2,
        "the integer arm of the render writers stands spelled in both writers"
    );
    assert_eq!(
        source
            .matches("JsonValue::BigInt(text) => out.push_str(text),")
            .count(),
        2,
        "the big integer arm of the render writers stands spelled in both writers"
    );
    assert_eq!(
        source
            .matches("JsonValue::Float(number) => out.push_str(&json_float_text(*number)),")
            .count(),
        2,
        "the float arm of the render writers stands spelled in both writers"
    );
    assert_eq!(
        source
            .matches("JsonValue::Text(text) => escape_string(text, out),")
            .count(),
        2,
        "the text arm of the render writers stands spelled in both writers"
    );
    assert_eq!(
        source
            .matches("sorted.sort_by(|a, b| a.0.cmp(&b.0));")
            .count(),
        2,
        "the object key sort step of the render writers stands spelled in both writers"
    );
}

/// The structural pin of slice 32. The two error types
/// of the errors source stand as siblings in the port,
/// the pack error standing beside the base error where
/// the Python original carried it as a subclass, and
/// the two types carry their shared shape in mirrored
/// spellings, the message field and the constructor and
/// the display reading and the error trait standing
/// spelled in both. This pin reads the errors source
/// and counts the four spellings, each standing in both
/// types and at no third seat. It fails on the day one
/// type its shape stands parted from the other at any
/// of the four spellings, and it passes on the tree
/// as landed, the
/// statement of the slice moving no code.
#[test]
fn test_the_twin_error_types_stand_mirrored_in_the_errors_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("errors.rs"),
    )
    .expect("the errors source stands in the tree");
    assert_eq!(
        source.matches("    message: String,").count(),
        2,
        "the message field of the twin error types stands spelled in both types"
    );
    assert_eq!(
        source
            .matches("    pub fn new(message: impl Into<String>) -> Self {")
            .count(),
        2,
        "the constructor of the twin error types stands spelled in both types"
    );
    assert_eq!(
        source.matches("impl std::error::Error for ").count(),
        2,
        "the error trait of the twin error types stands implemented for both types"
    );
    assert_eq!(
        source.matches("f.write_str(&self.message)").count(),
        2,
        "the display reading of the twin error types stands spelled in both types"
    );
}
