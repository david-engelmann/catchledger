//! The seed and template and annex pins of obligation PORT3-5,
//! re-expressed from the retired Python setup suite with the same
//! files and the same sentences, the gate sentence standing at
//! the wording obligation PORT3-6 gives it.

use std::path::PathBuf;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).expect("a standing file of the tree reads")
}

#[test]
fn test_seed_files_present() {
    let readme = read("README.md");
    let licence = read("LICENSE");
    let gitignore = read(".gitignore");
    assert!(readme.contains("# catchledger"));
    assert!(readme.contains("The scorecard every gate should carry"));
    assert!(readme.contains("## Licence"));
    assert!(licence.contains("MIT License"));
    assert!(gitignore.contains("__pycache__/"));
    assert!(gitignore.contains(".env"));
}

#[test]
fn test_readme_records_the_closed_round_count() {
    let readme = read("README.md");
    assert!(readme.contains("Closed rounds stand at 4 of 100"));
}

#[test]
fn test_agents_template_and_annex() {
    let agents = read("AGENTS.md");
    let marker = "<!-- repo annex below this line -->";
    assert!(agents.starts_with("# AGENTS.md"));
    assert!(agents.contains(
        "This repo is one of the repos in David Engelmann's 39 repo storefront perfection program"
    ));
    assert!(agents.contains(marker));
    let annex = agents
        .split(marker)
        .nth(1)
        .expect("the annex stands below its marker");
    let sentences = [
        "Catchledger prices checks, secondbill prices work.",
        "This repo keeps the per-check scorecard and publishes the evidence pack format the verdict tools read.",
        "All ingestion fixtures are synthetic and tests never touch the network.",
        "The local gate is cargo test.",
        "The closed round count stands in the README and moves only at a closure.",
    ];
    let positions: Vec<usize> = sentences
        .iter()
        .map(|sentence| {
            annex
                .find(sentence)
                .unwrap_or_else(|| panic!("the annex carries the sentence {sentence}"))
        })
        .collect();
    let mut sorted = positions.clone();
    sorted.sort_unstable();
    assert_eq!(positions, sorted, "the annex sentences stand in order");
}
