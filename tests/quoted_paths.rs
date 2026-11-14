//! The pins of slice 5, the quoted path decoding, over
//! synthetic histories and a synthetic local repository the
//! suite writes itself.

use std::path::{Path, PathBuf};
use std::process::Command;

use catchledger::ingest::{parse_git_log, read_git_history_from_clone};

const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn history(change_lines: &str) -> String {
    format!(
        "commit {HASH_A}\nparents \ndate 2026-01-01T00:00:00Z\nsubject Synthetic commit\n{change_lines}\n"
    )
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "catchledger-quoted-paths-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("the scratch directory stands");
    dir
}

fn git(dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} stands: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_a_quoted_utf8_path_is_decoded_in_the_records() {
    let records =
        parse_git_log(&history("A\t\"src/caf\\303\\251.py\"")).expect("the history parses");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].changes.len(), 1);
    assert_eq!(records[0].changes[0].paths, vec!["src/café.py".to_string()]);
}

#[test]
fn test_a_quoted_path_with_an_escaped_quote_is_decoded() {
    let records =
        parse_git_log(&history("A\t\"src/with\\\"quote.py\"")).expect("the history parses");
    assert_eq!(
        records[0].changes[0].paths,
        vec!["src/with\"quote.py".to_string()]
    );
}

#[test]
fn test_an_unquoted_path_reads_as_it_reads_today() {
    let records = parse_git_log(&history("A\tsrc/with space.py\nM\tsrc/plain.py"))
        .expect("the history parses");
    assert_eq!(
        records[0].changes[0].paths,
        vec!["src/with space.py".to_string()]
    );
    assert_eq!(
        records[0].changes[1].paths,
        vec!["src/plain.py".to_string()]
    );
}

#[test]
fn test_a_rename_line_decodes_both_of_its_paths() {
    let records = parse_git_log(&history(
        "R100\t\"src/caf\\303\\251.py\"\t\"src/th\\303\\251.py\"",
    ))
    .expect("the history parses");
    assert_eq!(
        records[0].changes[0].paths,
        vec!["src/café.py".to_string(), "src/thé.py".to_string()]
    );
}

#[test]
fn test_a_malformed_quoted_path_is_refused() {
    for line in [
        "A\t\"src/unterminated.py",
        "A\t\"src/bad\\qescape.py\"",
        "A\t\"src/short\\30.py\"",
        "A\t\"src/past\\777byte.py\"",
        "A\t\"src/inner\"quote.py\"",
    ] {
        let error = parse_git_log(&history(line)).expect_err("a malformed quoting refuses");
        assert_eq!(
            error.message(),
            "a Git history change line is malformed",
            "the line {line} refuses in the standing words"
        );
    }
}

#[test]
fn test_a_quoted_path_decoding_to_invalid_utf8_is_refused() {
    let error = parse_git_log(&history("A\t\"src/bad\\377.py\""))
        .expect_err("bytes past the UTF-8 reading refuse");
    assert_eq!(error.message(), "a Git history change line is malformed");
}

#[test]
fn test_clone_and_export_agree_on_quoted_paths() {
    let dir = scratch("parity");
    let repo = dir.join("repo");
    std::fs::create_dir_all(repo.join("src")).expect("the repo stands");
    git(&repo, &["init", "-q", "."]);
    git(
        &repo,
        &["config", "user.email", "synthetic@example.invalid"],
    );
    git(&repo, &["config", "user.name", "synthetic"]);
    std::fs::write(repo.join("src/café.py"), "x").expect("the file writes");
    git(&repo, &["add", "-A"]);
    git(&repo, &["commit", "-qm", "one"]);
    let (records, _digest) = read_git_history_from_clone(&repo).expect("the clone reads");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].changes[0].paths, vec!["src/café.py".to_string()]);
    let output = Command::new("git")
        .arg("-C")
        .arg(&repo)
        .args([
            "log",
            "--reverse",
            "--topo-order",
            "-M",
            "--name-status",
            "--pretty=format:commit %H%nparents %P%ndate %cI%nsubject %s",
        ])
        .output()
        .expect("git runs");
    assert!(output.status.success(), "the export command stands");
    let text = String::from_utf8(output.stdout).expect("the export is text");
    assert!(
        text.contains("\"src/caf\\303\\251.py\""),
        "the export itself carries the quoted form the parser decodes: {text}"
    );
    let exported = parse_git_log(&text).expect("the export parses");
    assert_eq!(exported.len(), records.len());
    assert_eq!(exported[0].changes, records[0].changes);
}
