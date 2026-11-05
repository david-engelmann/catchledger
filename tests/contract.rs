//! The contract pins of obligation PORT3-5. Every test name the
//! contract carries stands disposed in the name mapping beside
//! these tests, and the mapping names only Rust tests that stand
//! in the tree and recorded sections and cases that stand in the
//! recordings files it names. The dash pin of the retired Python
//! contract suite stands here in its Rust form.

use std::collections::BTreeSet;
use std::path::PathBuf;

use catchledger::jsonvalue::JsonValue;
use catchledger::read_json_document;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).expect("a standing file of the tree reads")
}

/// The test names a contract carries, read the way the retired
/// Python pin read them, a name being the word test_ and the
/// lowercase letters and digits and underscores that follow it,
/// standing at a word boundary.
fn contract_names(contract: &str) -> BTreeSet<String> {
    let chars: Vec<char> = contract.chars().collect();
    let mut names = BTreeSet::new();
    let mut at = 0;
    while at < chars.len() {
        let boundary = at == 0 || !(chars[at - 1].is_ascii_alphanumeric() || chars[at - 1] == '_');
        if boundary && chars[at..].starts_with(&['t', 'e', 's', 't', '_']) {
            let mut end = at + 5;
            while end < chars.len()
                && (chars[end].is_ascii_lowercase()
                    || chars[end].is_ascii_digit()
                    || chars[end] == '_')
            {
                end += 1;
            }
            if end > at + 5 {
                names.insert(chars[at..end].iter().collect());
            }
            at = end;
        } else {
            at += 1;
        }
    }
    names
}

struct Disposition {
    name: String,
    kind: String,
    file: String,
    target: String,
}

fn mapping() -> Vec<Disposition> {
    let text = read("tests/contract_name_map.txt");
    let mut lines = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line.split(" | ").collect();
        assert_eq!(
            fields.len(),
            4,
            "each mapping line carries a name and a kind and a file and a target"
        );
        lines.push(Disposition {
            name: fields[0].to_string(),
            kind: fields[1].to_string(),
            file: fields[2].to_string(),
            target: fields[3].to_string(),
        });
    }
    lines
}

fn recording_cases(rel: &str) -> Vec<JsonValue> {
    let text = read(rel);
    let document =
        read_json_document(&text, "a recordings file").expect("a recordings file parses");
    document
        .get("cases")
        .and_then(|cases| cases.as_list())
        .expect("a recordings file the mapping names carries its cases")
        .to_vec()
}

#[test]
fn test_every_contract_name_is_disposed() {
    let contract = read("CONTRACT.md");
    let names = contract_names(&contract);
    assert!(!names.is_empty(), "the contract carries test names");
    let lines = mapping();
    let mapped: BTreeSet<&str> = lines.iter().map(|line| line.name.as_str()).collect();
    for name in &names {
        assert!(
            mapped.contains(name.as_str()),
            "the contract name {name} stands without a disposition"
        );
    }
    for line in &lines {
        assert!(
            names.contains(&line.name),
            "the mapping disposes {} which the contract does not carry",
            line.name
        );
        match line.kind.as_str() {
            "rust-test" | "re-expressed" => {
                let source = read(&line.file);
                let needle = format!("fn {}(", line.target);
                assert!(
                    source.contains(&needle),
                    "the mapping names the Rust test {} in {} and it does not stand there",
                    line.target,
                    line.file
                );
            }
            "recording-section" => {
                let cases = recording_cases(&line.file);
                assert!(
                    cases.iter().any(|case| {
                        case.get("kind").and_then(|value| value.as_text())
                            == Some(line.target.as_str())
                            || case.get("section").and_then(|value| value.as_text())
                                == Some(line.target.as_str())
                    }),
                    "the mapping names the recorded section {} in {} and it does not stand there",
                    line.target,
                    line.file
                );
            }
            "recording-case" => {
                let cases = recording_cases(&line.file);
                assert!(
                    cases.iter().any(|case| {
                        case.get("name").and_then(|value| value.as_text())
                            == Some(line.target.as_str())
                    }),
                    "the mapping names the recorded case {} in {} and it does not stand there",
                    line.target,
                    line.file
                );
            }
            other => panic!("the mapping carries an unknown disposition kind {other}"),
        }
    }
}

#[test]
fn test_contract_has_no_forbidden_dashes() {
    let contract = read("CONTRACT.md");
    assert_eq!(contract.matches('\u{2014}').count(), 0);
    assert_eq!(contract.matches('\u{2013}').count(), 0);
}
