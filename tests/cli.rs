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
/// the could not be read wording at its definition alone
/// and the not valid wording at its definition and at
/// the refusal helper, the file reading arm of each
/// wording standing retired by slice 33, and this pin
/// reads those sources and counts the standings, so a
/// reading place that
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
        1,
        "the could not be read wording stands at its definition alone in the reader source"
    );
    assert_eq!(
        reader_source.matches("not_valid_json_wording").count(),
        2,
        "the not valid JSON wording stands at its definition and at its refusal helper seat"
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

/// The structural pin of slice 33. The file reading
/// of the reader, retired by this slice, stands at no
/// seat in the reader source or in the crate root, and
/// this pin reads those two sources by the manifest
/// directory route and counts the standings of its
/// name in each. It fails on the contract tree of this
/// round in the designed shape, where the function
/// still stands defined in the reader and re-exported
/// at the root, and it passes on the build tree, and
/// it fails on the day the name stands respelled at
/// either seat, whether as a restored function or as
/// a fresh reading.
#[test]
fn test_the_file_reading_function_stands_at_no_seat_in_the_reader_or_the_crate_root() {
    let source_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let reader_source = std::fs::read_to_string(source_dir.join("reader.rs"))
        .expect("the reader source stands in the tree");
    assert_eq!(
        reader_source.matches("read_json_file").count(),
        0,
        "the file reading stands at no seat in the reader source"
    );
    let root_source = std::fs::read_to_string(source_dir.join("lib.rs"))
        .expect("the crate root stands in the tree");
    assert_eq!(
        root_source.matches("read_json_file").count(),
        0,
        "the file reading stands at no seat in the crate root"
    );
}

/// The behavioural pin of slice 33. The document
/// reading of the reader stands whole at the library
/// door after the retirement of the file reading
/// beside it, and this pin drives that reading over a
/// sound document and asserts the value it returns
/// through the rendering, and over an invalid text
/// and asserts the refusal carries the not valid JSON
/// wording under its label, and over a text carrying
/// a key twice and asserts the refusal names the key
/// under its label. It passes on the tree as landed,
/// binding the surface the retirement must not move,
/// and it fails on the day the removal or any later
/// hand parts the document reading or either wording
/// or the duplicate refusal.
#[test]
fn test_the_document_reading_stands_whole_at_the_library_door() {
    let document = read_json_document("{\"count\": 2, \"name\": \"ledger\"}", "checked document")
        .expect("a sound document reads at the library door");
    assert_eq!(
        catchledger::render_json(&document),
        "{\n  \"count\": 2,\n  \"name\": \"ledger\"\n}\n",
        "the document reading returns the document whole"
    );
    let error = read_json_document("not a document", "checked document")
        .expect_err("an invalid text refuses at the library door");
    assert_eq!(
        error.message(),
        "the checked document is not valid JSON",
        "the invalid text refuses in the not valid JSON wording under its label"
    );
    let error = read_json_document(
        "{\"name\": \"ledger\", \"name\": \"again\"}",
        "checked document",
    )
    .expect_err("a key carried twice refuses at the library door");
    assert_eq!(
        error.message(),
        "the checked document carries the key name more than once",
        "the duplicate key refusal names the key under its label"
    );
}

/// The structural pin of slice 34. The two token
/// price defaults of the door stand spelled as the
/// one fallback text at two sites in the main
/// source, one site under each price flag, with no
/// constant naming either default, and this pin
/// reads the main source by the manifest directory
/// route and counts the fallback arm at exactly two
/// standings and the reading seat of each price flag
/// at exactly one standing. It fails on the day
/// either default stands respelled at its site, on
/// the day one site stands removed, and on the day
/// a third spelling of the arm appears, and it
/// passes on the tree as landed, the statement of
/// the slice moving no code. A text count cannot
/// prove that the two arms stand paired to their
/// own flags rather than crossed, and the pin
/// answers that limit by pairing the arm count with
/// the two seat counts and leaving the last step to
/// the words of the section and the diff reviews,
/// the behavioural pins of this slice proving the
/// pairing at the door in both mixed standings.
#[test]
fn test_the_token_price_default_text_stands_spelled_twice_in_the_main_source() {
    let source = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join("main.rs"),
    )
    .expect("the main source stands in the tree");
    assert_eq!(
        source.matches("None => \"0\".to_string(),").count(),
        2,
        "the token price default text stands spelled at the two price sites of the main source"
    );
    assert_eq!(
        source
            .matches("values.get(\"price-per-input-token\")")
            .count(),
        1,
        "the input price flag stands read at one seat in the main source"
    );
    assert_eq!(
        source
            .matches("values.get(\"price-per-output-token\")")
            .count(),
        1,
        "the output price flag stands read at one seat in the main source"
    );
}

/// The first door pin of slice 35. The repository label
/// flag standing as UTF-8 text carrying no characters
/// falls to the document label exactly as an absent flag
/// does, at every shape the label reading of PORT2-3
/// carries, and this pin drives the door with an empty
/// label over three export documents, one carrying the
/// repo field standing, one carrying the full name
/// inside a repository object with no field ahead of
/// it, and one carrying no label field at all, and
/// reads the label the written ledger carries at each
/// cell. It fails on the day the empty flag stands
/// taken as the label itself, the ledger carrying an
/// empty label at all three cells, and on the day the
/// empty flag stands refused, the door writing nothing
/// at all three cells, and it passes on the tree as
/// landed, the statement of the slice moving no code.
#[test]
fn test_an_empty_repo_label_flag_reads_the_document_label() {
    let runs = recorded_input("deterministic outputs over the examples", "runs.json");
    let log = recorded_input("deterministic outputs over the examples", "gitlog.txt");
    let full_name_runs = runs.replacen(
        "{\"repo\":\"synthetic-ledger-repo\",",
        "{\"repository\":{\"full_name\":\"synthetic/full-name\"},",
        1,
    );
    assert!(
        full_name_runs.starts_with("{\"repository\":"),
        "the full name document carries the repository object at its head"
    );
    let bare_runs = runs.replacen("{\"repo\":\"synthetic-ledger-repo\",", "{", 1);
    assert!(
        bare_runs.starts_with("{\"workflow_runs\":"),
        "the bare document carries no label field at its head"
    );
    let label_of = |name: &str, document: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let runs_path = dir.join("runs.json");
        std::fs::write(&runs_path, document.as_bytes()).expect("the export stands");
        let log_path = dir.join("gitlog.txt");
        std::fs::write(&log_path, log.as_bytes()).expect("the history stands");
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg("--ci-history")
            .arg(&runs_path)
            .arg("--git-log")
            .arg(&log_path)
            .arg("--repo-label")
            .arg("")
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
            "an empty label flag runs the door"
        );
        let ledger_text =
            std::fs::read_to_string(dir.join("out/ledger.json")).expect("the ledger stands");
        let ledger = read_json_document(&ledger_text, "ledger").expect("the ledger parses");
        text_of(ledger.get("repo").expect("the ledger carries its label")).to_string()
    };
    assert_eq!(
        label_of("catchledger-cli-empty-label-repo", &runs),
        "synthetic-ledger-repo",
        "an empty label flag reads the repo field of the document"
    );
    assert_eq!(
        label_of("catchledger-cli-empty-label-full-name", &full_name_runs),
        "synthetic/full-name",
        "an empty label flag reads the full name inside the repository object"
    );
    assert_eq!(
        label_of("catchledger-cli-empty-label-bare", &bare_runs),
        "unknown",
        "an empty label flag reads the word unknown where no label field stands"
    );
}

/// The second door pin of slice 35. The emptiness the
/// label resolution names is of the flag word as the
/// operating system handed it over, ahead of any
/// stripping, and a flag word of whitespace alone
/// carries characters and takes the label verbatim,
/// and this pin drives the door with a label of three
/// spaces over the recorded export and with a label
/// of a single tab over the recorded export and with
/// a label of a single newline over the recorded
/// export, and reads the label the written ledger
/// and the written report carry at each cell, the
/// report carrying the newline word in its own
/// spelling of that character, a space standing in
/// its place. It fails on the day the flag word
/// stands stripped ahead of the emptiness test, the
/// spaces and the tab and the newline falling to the
/// document label, and on the day a strip confined
/// to the tab alone stands ahead of that test, the
/// tab falling while the spaces and the newline
/// stand, and on the day a strip confined to the
/// newline alone stands ahead of that test, the
/// newline falling while the spaces and the tab
/// stand, and it passes on the tree as landed, the
/// statement of the slice moving no code.
#[test]
fn test_a_repo_label_flag_of_spaces_alone_stands_verbatim() {
    let runs = recorded_input("deterministic outputs over the examples", "runs.json");
    let log = recorded_input("deterministic outputs over the examples", "gitlog.txt");
    let verbatim_of = |name: &str, word: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
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
            .arg(word)
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
            "a label of whitespace alone runs the door"
        );
        let ledger_text =
            std::fs::read_to_string(dir.join("out/ledger.json")).expect("the ledger stands");
        let ledger = read_json_document(&ledger_text, "ledger").expect("the ledger parses");
        let report_text =
            std::fs::read_to_string(dir.join("out/report.md")).expect("the report stands");
        (
            text_of(ledger.get("repo").expect("the ledger carries its label")).to_string(),
            report_text,
        )
    };
    let (label, report) = verbatim_of("catchledger-cli-spaces-label", "   ");
    assert_eq!(
        label, "   ",
        "a label of spaces alone stands verbatim in the ledger"
    );
    assert!(
        report.contains("Repository    \n"),
        "a label of spaces alone stands verbatim in the report"
    );
    let (label, report) = verbatim_of("catchledger-cli-tab-label", "\t");
    assert_eq!(
        label, "\t",
        "a label of a single tab stands verbatim in the ledger"
    );
    assert!(
        report.contains("Repository \t\n"),
        "a label of a single tab stands verbatim in the report"
    );
    let (label, report) = verbatim_of("catchledger-cli-newline-label", "\n");
    assert_eq!(
        label, "\n",
        "a label of a single newline stands verbatim in the ledger"
    );
    assert!(
        report.contains("Repository  \n"),
        "a label of a single newline stands verbatim in the report"
    );
}

/// The first door pin of slice 36. An ambiguous long
/// flag prefix refuses the reading at the point it
/// stands, ahead of any help flag standing later in
/// the line, and this pin drives the door at one cell
/// for each ambiguous family at its longest ambiguous
/// spelling, the spellings --c and --check- and
/// --git- and --p and --price-per- and --repo and
/// --token-, each followed by a value shaped word and
/// then by the help flag, and reads the usage refusal
/// at every cell, the refusal standing as exit code 2
/// with the usage line on standard error and nothing
/// on standard output and nothing written. It fails
/// on the day a reading resolved an ambiguous prefix
/// to a first match, the scan proceeding at that cell
/// and the later help answering, and it passes on the
/// tree as landed, the statement of the slice moving
/// no code.
#[test]
fn test_ambiguous_long_flag_prefixes_refuse_at_the_point_they_stand() {
    let refusal_of = |name: &str, spelling: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(spelling)
            .arg("x")
            .arg("--help")
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "an ambiguous prefix refuses the reading"
        );
        assert_eq!(
            output.stderr,
            USAGE_LINE.as_bytes(),
            "an ambiguous prefix writes the usage line to standard error"
        );
        assert!(
            output.stdout.is_empty(),
            "an ambiguous prefix writes nothing to standard output"
        );
        assert_eq!(
            std::fs::read_dir(&dir)
                .expect("the pin directory reads")
                .count(),
            0,
            "an ambiguous prefix writes nothing"
        );
    };
    refusal_of("catchledger-cli-ambiguous-c", "--c");
    refusal_of("catchledger-cli-ambiguous-check", "--check-");
    refusal_of("catchledger-cli-ambiguous-git", "--git-");
    refusal_of("catchledger-cli-ambiguous-p", "--p");
    refusal_of("catchledger-cli-ambiguous-price", "--price-per-");
    refusal_of("catchledger-cli-ambiguous-repo", "--repo");
    refusal_of("catchledger-cli-ambiguous-token", "--token-");
}

/// The second door pin of slice 36. A long flag
/// prefix at or past the boundary of its name reads
/// as that name, and this pin drives the door at one
/// cell for each of the seventeen boundaries in the
/// help adjacency shape, the boundary prefix followed
/// by its value and then by the help flag, and reads
/// the help answer at every cell, and at the three
/// help prefixes h and he and hel standing alone
/// reads the help answer, and at two equals form
/// cells at the output boundary and the token cost
/// check boundary reads the help answer, and at ten
/// further cells at the boundary spellings the whole
/// invocation does not carry, the two token flags
/// and the two price flags and the four check flags
/// and the repository history source and the output,
/// each standing directly ahead of the help flag
/// with no value between, reads the usage refusal,
/// and at one whole invocation cell spelling the
/// flags of a standing run at their boundary
/// prefixes over the
/// recorded export and the recorded history reads
/// the run completing with its outputs standing
/// written and the ledger carrying the label word
/// the boundary spelling gave it. It fails on the
/// day a boundary stood one letter longer at any
/// name, that name its cell refusing or standing set
/// aside with the help not answering, and on the day
/// a reading dropped the help flag from the prefix
/// space, the three help cells failing, and on the
/// day a boundary spelling stood unknown to the
/// reading, that spelling standing set aside with
/// its direct cell answering the help flag where
/// the pin reads the usage refusal, and it passes
/// on the tree as landed, the statement of the
/// slice moving no code.
#[test]
fn test_unique_long_flag_prefixes_resolve_to_their_flags() {
    let help_of = |name: &str, args: &[&str]| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .args(args)
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(0),
            "a resolving prefix reaches the help answer"
        );
        assert!(
            !output.stdout.is_empty(),
            "a resolving prefix answers the help flag on standard output"
        );
        assert!(
            output.stderr.is_empty(),
            "a resolving prefix writes nothing to standard error"
        );
    };
    let boundaries = [
        "--ci",
        "--git-l",
        "--git-r",
        "--ca",
        "--o",
        "--repor",
        "--pa",
        "--repo-",
        "--co",
        "--token-u",
        "--token-o",
        "--price-per-i",
        "--price-per-o",
        "--check-t",
        "--check-r",
        "--check-p",
        "--check-l",
    ];
    for (index, spelling) in boundaries.iter().enumerate() {
        let name = format!("catchledger-cli-boundary-{index}");
        help_of(&name, &[*spelling, "x", "--help"]);
    }
    help_of("catchledger-cli-help-h", &["--h"]);
    help_of("catchledger-cli-help-he", &["--he"]);
    help_of("catchledger-cli-help-hel", &["--hel"]);
    help_of("catchledger-cli-equals-output", &["--o=x", "--help"]);
    help_of("catchledger-cli-equals-check", &["--check-t=x", "--help"]);
    let refusal_of = |name: &str, spelling: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(spelling)
            .arg("--help")
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "a boundary spelling standing directly ahead of the help flag refuses the reading"
        );
        assert_eq!(
            output.stderr,
            USAGE_LINE.as_bytes(),
            "a boundary spelling standing directly ahead of the help flag writes the usage line to standard error"
        );
        assert!(
            output.stdout.is_empty(),
            "a boundary spelling standing directly ahead of the help flag writes nothing to standard output"
        );
        assert_eq!(
            std::fs::read_dir(&dir)
                .expect("the pin directory reads")
                .count(),
            0,
            "a boundary spelling standing directly ahead of the help flag writes nothing"
        );
    };
    let directs = [
        "--token-u",
        "--token-o",
        "--price-per-i",
        "--price-per-o",
        "--check-t",
        "--check-r",
        "--check-p",
        "--check-l",
        "--git-r",
        "--o",
    ];
    for (index, spelling) in directs.iter().enumerate() {
        let name = format!("catchledger-cli-direct-{index}");
        refusal_of(&name, spelling);
    }
    let runs = recorded_input("deterministic outputs over the examples", "runs.json");
    let log = recorded_input("deterministic outputs over the examples", "gitlog.txt");
    let canary = recorded_input("deterministic outputs over the examples", "canary.json");
    let dir = pin_dir("catchledger-cli-boundary-run");
    let _scratch = ScratchDir::hold(&dir);
    let runs_path = dir.join("runs.json");
    std::fs::write(&runs_path, runs.as_bytes()).expect("the export stands");
    let log_path = dir.join("gitlog.txt");
    std::fs::write(&log_path, log.as_bytes()).expect("the history stands");
    let canary_path = dir.join("canary.json");
    std::fs::write(&canary_path, canary.as_bytes()).expect("the results stand");
    let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
        .arg("--ci")
        .arg(&runs_path)
        .arg("--git-l")
        .arg(&log_path)
        .arg("--ca")
        .arg(&canary_path)
        .arg("--o")
        .arg(dir.join("out/ledger.json"))
        .arg("--repor")
        .arg(dir.join("out/report.md"))
        .arg("--pa")
        .arg(dir.join("out/packs"))
        .arg("--repo-")
        .arg("boundary-label")
        .arg("--co")
        .arg("0.01")
        .output()
        .expect("the binary runs");
    assert_eq!(
        output.status.code(),
        Some(0),
        "a run spelled at boundary prefixes completes"
    );
    let ledger_text =
        std::fs::read_to_string(dir.join("out/ledger.json")).expect("the ledger stands");
    let ledger = read_json_document(&ledger_text, "ledger").expect("the ledger parses");
    assert_eq!(
        text_of(ledger.get("repo").expect("the ledger carries its label")),
        "boundary-label",
        "the boundary spelling of the label flag gives the ledger its label"
    );
    assert!(
        dir.join("out/report.md").exists(),
        "the boundary run writes its report"
    );
    assert!(
        dir.join("out/packs").is_dir(),
        "the boundary run writes its packs"
    );
}

/// The third door pin of slice 36. A prefix of the
/// help flag carrying an equals value refuses, as
/// the help flag carrying an explicit value refuses,
/// and this pin drives the door at the spellings
/// --hel and --help each carrying an equals value
/// and reads the usage refusal at both cells. It
/// fails on the day a reading answered a valued help
/// prefix with the help text, and it passes on the
/// tree as landed, the statement of the slice moving
/// no code.
#[test]
fn test_a_prefix_of_the_help_flag_carrying_a_value_refuses() {
    let refusal_of = |name: &str, spelling: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(spelling)
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "a valued help prefix refuses the reading"
        );
        assert_eq!(
            output.stderr,
            USAGE_LINE.as_bytes(),
            "a valued help prefix writes the usage line to standard error"
        );
        assert!(
            output.stdout.is_empty(),
            "a valued help prefix writes nothing to standard output"
        );
    };
    refusal_of("catchledger-cli-valued-help-prefix", "--hel=x");
    refusal_of("catchledger-cli-valued-help-full", "--help=x");
}

/// The first door pin of slice 37. A single dash
/// token whose second byte is the letter h and whose
/// third byte is not an equals sign answers with the
/// help at the point it stands, the letters after
/// the h never read, and this pin drives the door at
/// the spellings -hx and -help and -h1 standing
/// alone and reads the help answer at every cell,
/// the answer standing as exit code 0 with the help
/// text on standard output. It fails on the day a
/// reading answered only the exact spelling -h, or
/// read the letters after the h and refused the
/// letters it does not know, and it passes on the
/// tree as landed, the statement of the slice moving
/// no code.
#[test]
fn test_single_dash_help_clusters_answer_with_the_help_text() {
    let help_of = |name: &str, spelling: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(spelling)
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(0),
            "a firing help cluster answers with the help"
        );
        assert!(
            !output.stdout.is_empty(),
            "a firing help cluster writes the help text to standard output"
        );
        assert!(
            output.stderr.is_empty(),
            "a firing help cluster writes nothing to standard error"
        );
    };
    help_of("catchledger-cli-single-dash-hx", "-hx");
    help_of("catchledger-cli-single-dash-help", "-help");
    help_of("catchledger-cli-single-dash-h1", "-h1");
    let help_pair_of = |name: &str, first: &str, second: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(first)
            .arg(second)
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(0),
            "a firing help cluster answers with the help at the point it stands"
        );
        assert!(
            !output.stdout.is_empty(),
            "a firing help cluster writes the help text to standard output"
        );
        assert!(
            output.stderr.is_empty(),
            "a firing help cluster writes nothing to standard error"
        );
    };
    help_pair_of(
        "catchledger-cli-single-dash-aside-then-cluster",
        "-x",
        "-hx",
    );
    help_pair_of(
        "catchledger-cli-single-dash-cluster-then-valued-help",
        "-hx",
        "--help=x",
    );
}

/// The second door pin of slice 37. Every other
/// single dash token stands set aside in the
/// deferred discipline of the scan and refuses at
/// the end of the scan, a help flag standing later
/// in the line still answering over it, and this
/// pin drives the door at the spellings -xh and -x
/// and the lone dash and -5 standing alone and
/// reads the usage refusal at every cell, the
/// refusal standing as exit code 2 with the usage
/// line on standard error and nothing on standard
/// output, and at the same four spellings each
/// followed by the long help flag and reads the
/// help answer at every cell. It fails on the day
/// a reading fired the help wherever the letter h
/// stands in a cluster, the reversed spelling cell
/// answering, and on the day a reading refused a
/// standing aside token at the point it stands,
/// the four help adjacency cells failing, and it
/// passes on the tree as landed, the statement of
/// the slice moving no code.
#[test]
fn test_single_dash_tokens_that_do_not_fire_the_help_refuse_at_the_end_of_the_scan() {
    let refusal_of = |name: &str, spelling: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(spelling)
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "a standing aside token refuses at the end of the scan"
        );
        assert_eq!(
            output.stderr,
            USAGE_LINE.as_bytes(),
            "a standing aside token writes the usage line to standard error"
        );
        assert!(
            output.stdout.is_empty(),
            "a standing aside token writes nothing to standard output"
        );
        assert_eq!(
            std::fs::read_dir(&dir)
                .expect("the pin directory reads")
                .count(),
            0,
            "a standing aside token writes nothing"
        );
    };
    let help_of = |name: &str, spelling: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(spelling)
            .arg("--help")
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(0),
            "a later help flag answers over a standing aside token"
        );
        assert!(
            !output.stdout.is_empty(),
            "a later help flag writes the help text to standard output"
        );
        assert!(
            output.stderr.is_empty(),
            "a later help flag writes nothing to standard error"
        );
    };
    refusal_of("catchledger-cli-standing-aside-xh", "-xh");
    refusal_of("catchledger-cli-standing-aside-x", "-x");
    refusal_of("catchledger-cli-standing-aside-lone", "-");
    refusal_of("catchledger-cli-standing-aside-neg", "-5");
    refusal_of("catchledger-cli-standing-aside-upper-h", "-H");
    help_of("catchledger-cli-standing-aside-xh-help", "-xh");
    help_of("catchledger-cli-standing-aside-x-help", "-x");
    help_of("catchledger-cli-standing-aside-lone-help", "-");
    help_of("catchledger-cli-standing-aside-neg-help", "-5");
    help_of("catchledger-cli-standing-aside-upper-h-help", "-H");
}

/// The third door pin of slice 37. A help cluster
/// carrying an equals sign directly after the h
/// stands set aside and does not refuse at the
/// point it stands, parting from the valued long
/// help forms, which refuse at their point, and
/// this pin drives the door at the spellings -h=x
/// and -h= standing alone and reads the usage
/// refusal at both cells, and at the same two
/// spellings each followed by the long help flag
/// and reads the help answer at both cells. It
/// fails on the day a reading refused a valued
/// help cluster at its point as the long form
/// refuses, the two help adjacency cells failing,
/// and on the day a reading fired a valued
/// cluster, the two standing alone cells failing,
/// and it passes on the tree as landed, the
/// statement of the slice moving no code.
#[test]
fn test_a_help_cluster_carrying_an_equals_value_stands_set_aside() {
    let refusal_of = |name: &str, spelling: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(spelling)
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "a valued help cluster refuses at the end of the scan"
        );
        assert_eq!(
            output.stderr,
            USAGE_LINE.as_bytes(),
            "a valued help cluster writes the usage line to standard error"
        );
        assert!(
            output.stdout.is_empty(),
            "a valued help cluster writes nothing to standard output"
        );
    };
    let help_of = |name: &str, spelling: &str| {
        let dir = pin_dir(name);
        let _scratch = ScratchDir::hold(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_catchledger"))
            .arg(spelling)
            .arg("--help")
            .current_dir(&dir)
            .output()
            .expect("the binary runs");
        assert_eq!(
            output.status.code(),
            Some(0),
            "a later help flag answers over a valued help cluster"
        );
        assert!(
            !output.stdout.is_empty(),
            "a later help flag writes the help text to standard output"
        );
        assert!(
            output.stderr.is_empty(),
            "a later help flag writes nothing to standard error"
        );
    };
    refusal_of("catchledger-cli-valued-cluster-x", "-h=x");
    refusal_of("catchledger-cli-valued-cluster-empty", "-h=");
    help_of("catchledger-cli-valued-cluster-x-help", "-h=x");
    help_of("catchledger-cli-valued-cluster-empty-help", "-h=");
}
