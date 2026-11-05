//! The catchledger command line over the Rust library.
//!
//! This shell is the closing slice of the port under obligations
//! PORT3-1 to PORT3-3. It carries the one flat door of the Python
//! command line, with the argument reading the contract probe
//! recorded. A reading the argument reader refuses exits with
//! code 2 and the short usage line of this shell on standard
//! error, and a refusal of the tool exits with code 2 and the one
//! refusal line, the catchledger error prefix and the wording, on
//! standard error, and a door that succeeds exits with code 0 in
//! silence. The arguments stand as the operating system hands them
//! over, their bytes whole, so a file name carrying a byte that is
//! not UTF-8 names the file it names.

use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::PathBuf;
use std::process::ExitCode;

use catchledger::digest::sha256_bytes;
use catchledger::ingest::{
    ci_repo_label, parse_ci_document, parse_git_log, read_git_history_from_clone,
};
use catchledger::jsonvalue::JsonValue;
use catchledger::ledger::{
    build_evidence_packs, build_ledger, validate_evidence_pack, LedgerInputs,
};
use catchledger::reader::read_json_document;
use catchledger::render::render_json;
use catchledger::report::render_markdown;
use catchledger::{validate_canary_document, CatchledgerError};

const USAGE: &str = "usage: catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --output PATH --report PATH --packs-dir DIR [--repo-label TEXT] [--cost-per-minute TEXT]";

const HELP: &str = "usage: catchledger --ci-history PATH (--git-log PATH | --git-repo PATH) [--canaryfield-results PATH] --output PATH --report PATH --packs-dir DIR [--repo-label TEXT] [--cost-per-minute TEXT]

build a conservative per check scorecard from local CI and Git history
";

struct FlagSpec {
    name: &'static str,
    required: bool,
}

const FLAGS: &[FlagSpec] = &[
    FlagSpec {
        name: "ci-history",
        required: true,
    },
    FlagSpec {
        name: "git-log",
        required: false,
    },
    FlagSpec {
        name: "git-repo",
        required: false,
    },
    FlagSpec {
        name: "canaryfield-results",
        required: false,
    },
    FlagSpec {
        name: "output",
        required: true,
    },
    FlagSpec {
        name: "report",
        required: true,
    },
    FlagSpec {
        name: "packs-dir",
        required: true,
    },
    FlagSpec {
        name: "repo-label",
        required: false,
    },
    FlagSpec {
        name: "cost-per-minute",
        required: false,
    },
];

/// The values of the door reading, each standing as the operating
/// system handed it over.
type Values = HashMap<&'static str, OsString>;

fn token_bytes(token: &OsString) -> &[u8] {
    token.as_os_str().as_bytes()
}

/// The negative number shapes the Python argument reader does not
/// treat as option strings, a minus then digits with an optional
/// fractional part.
fn is_negative_number(token: &[u8]) -> bool {
    if token.first() != Some(&b'-') {
        return false;
    }
    let body = &token[1..];
    if body.is_empty() {
        return false;
    }
    if body.iter().all(|byte| byte.is_ascii_digit()) {
        return true;
    }
    match body.iter().position(|byte| *byte == b'.') {
        Some(dot) => {
            let (whole, fraction) = (&body[..dot], &body[dot + 1..]);
            whole.iter().all(|byte| byte.is_ascii_digit())
                && !fraction.is_empty()
                && fraction.iter().all(|byte| byte.is_ascii_digit())
        }
        None => false,
    }
}

/// Whether a token standing after a flag reads as a further option
/// string to the argument reader, in which case the flag stands
/// without its value.
fn looks_like_option(token: &[u8]) -> bool {
    token.len() > 1 && token[0] == b'-' && !is_negative_number(token)
}

/// Whether a single dash token is the help flag standing first in
/// a cluster of short flags. The Python argument reader expands a
/// cluster letter by letter, so the help flag fires where it
/// stands first, ahead of any letter it does not know, and a
/// cluster that carries an equals sign after the help letter is
/// the help flag carrying an explicit value, which that reader
/// refuses rather than answers.
fn is_help_cluster(token: &[u8]) -> bool {
    token.len() > 1
        && token[0] == b'-'
        && token[1] != b'-'
        && !is_negative_number(token)
        && token[1] == b'h'
        && (token.len() == 2 || token[2] != b'=')
}

enum LongMatch {
    Flag(usize),
    Help,
    Ambiguous,
    Unknown,
}

/// Match a long flag name against the flags of the door and the
/// help flag, exactly first and then by a prefix that names one
/// alone.
fn match_long(name: &[u8]) -> LongMatch {
    if name.is_empty() {
        return LongMatch::Unknown;
    }
    if name == b"help" {
        return LongMatch::Help;
    }
    for (index, spec) in FLAGS.iter().enumerate() {
        if spec.name.as_bytes() == name {
            return LongMatch::Flag(index);
        }
    }
    let mut found: Option<LongMatch> = None;
    let mut ambiguous = false;
    if b"help".starts_with(name) {
        found = Some(LongMatch::Help);
    }
    for (index, spec) in FLAGS.iter().enumerate() {
        if spec.name.as_bytes().starts_with(name) {
            if found.is_some() {
                ambiguous = true;
            }
            found = Some(LongMatch::Flag(index));
        }
    }
    if ambiguous {
        LongMatch::Ambiguous
    } else {
        found.unwrap_or(LongMatch::Unknown)
    }
}

enum Scan {
    Help,
    UsageError,
    Parsed(Values),
}

/// Read the arguments of the door the way the Python argument
/// reader reads them. An ambiguous prefix and a flag standing
/// without its value refuse the reading at the point they stand,
/// and so does the second of the two history source flags, whose
/// conflict the Python reader raises as it takes the second one.
/// A word or flag the door does not take is set aside and refuses
/// the reading once the scan ends, so a help flag standing after
/// one still answers. The separator word ends the option reading
/// and every word after it is set aside the same way. A flag given
/// more than once keeps its last value. A single dash token reads
/// as a cluster of short flags, the help flag firing where it
/// stands first in one.
fn scan(args: &[OsString]) -> Scan {
    let mut values: Values = HashMap::new();
    let mut refused_at_end = false;
    let mut after_separator = false;
    let mut position = 0;
    while position < args.len() {
        let token = &args[position];
        let tok = token_bytes(token);
        if after_separator {
            refused_at_end = true;
            position += 1;
            continue;
        }
        if tok == b"--" {
            after_separator = true;
            refused_at_end = true;
            position += 1;
            continue;
        }
        if tok == b"-h" || is_help_cluster(tok) {
            return Scan::Help;
        }
        if let Some(long) = tok.strip_prefix(b"--") {
            let (name, inline) = match long.iter().position(|byte| *byte == b'=') {
                Some(equal) => (
                    &long[..equal],
                    Some(OsString::from_vec(long[equal + 1..].to_vec())),
                ),
                None => (long, None),
            };
            match match_long(name) {
                LongMatch::Help => {
                    if inline.is_some() {
                        return Scan::UsageError;
                    }
                    return Scan::Help;
                }
                LongMatch::Ambiguous => return Scan::UsageError,
                LongMatch::Unknown => {
                    refused_at_end = true;
                    position += 1;
                }
                LongMatch::Flag(index) => {
                    let spec = &FLAGS[index];
                    let value = match inline {
                        Some(value) => {
                            position += 1;
                            value
                        }
                        None => match args.get(position + 1) {
                            Some(value) if !looks_like_option(token_bytes(value)) => {
                                let value = value.clone();
                                position += 2;
                                value
                            }
                            _ => return Scan::UsageError,
                        },
                    };
                    // The two history sources exclude each other, and
                    // the Python reader raises that conflict as it
                    // takes the second one, ahead of any help flag
                    // standing later in the line.
                    let other = match spec.name {
                        "git-log" => Some("git-repo"),
                        "git-repo" => Some("git-log"),
                        _ => None,
                    };
                    if let Some(other) = other {
                        if values.contains_key(other) {
                            return Scan::UsageError;
                        }
                    }
                    values.insert(spec.name, value);
                }
            }
            continue;
        }
        refused_at_end = true;
        position += 1;
    }
    if refused_at_end {
        return Scan::UsageError;
    }
    for spec in FLAGS {
        if spec.required && !values.contains_key(spec.name) {
            return Scan::UsageError;
        }
    }
    if values.contains_key("git-log") == values.contains_key("git-repo") {
        return Scan::UsageError;
    }
    Scan::Parsed(values)
}

fn need<'a>(values: &'a Values, name: &str) -> &'a OsString {
    values
        .get(name)
        .expect("a required flag stands once the reading accepts")
}

fn path_of(values: &Values, name: &str) -> PathBuf {
    PathBuf::from(need(values, name))
}

/// The text of the price word. The shell reads three classes of
/// word and this helper carries only the price. The path words
/// never pass through it, standing as bytes for the filesystem
/// end to end. The repository label word stands apart in the
/// door, refused there where it is not UTF-8 text, because the
/// label flows into the outputs as data no grammar reads. The
/// price word a decimal grammar reads, and no price that grammar
/// accepts can contain the replacement character, so a word
/// carrying a byte that is not UTF-8 reads here with that
/// character standing in it and refuses as no price, while a
/// word that stands whole as UTF-8 passes through unchanged.
fn word_text(value: &OsString) -> String {
    value.to_string_lossy().into_owned()
}

fn refusal(error: CatchledgerError) -> String {
    error.message().to_string()
}

/// Read one JSON document under its label exactly as the Python
/// reader does, the bytes as they stand on disk, refusing a read
/// failure and a decode failure in the reader wordings, and
/// returning the document beside the digest of its bytes.
fn read_document(path: &std::path::Path, label: &str) -> Result<(JsonValue, String), String> {
    let data = std::fs::read(path).map_err(|_| format!("the {label} could not be read"))?;
    let digest = sha256_bytes(&data);
    let text = String::from_utf8(data).map_err(|_| format!("the {label} is not valid JSON"))?;
    let document = read_json_document(&text, label).map_err(refusal)?;
    Ok((document, digest))
}

/// Write one output file the way the Python write helper does,
/// creating the directory path where it does not stand, with the
/// one refusal wording every write failure carries.
fn write_text(path: &std::path::Path, text: &str) -> Result<(), String> {
    let outcome = (|| -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        std::fs::write(path, text)
    })();
    outcome.map_err(|_| "an output could not be written".to_string())
}

fn door(values: &Values) -> Result<(), String> {
    let (ci_document, ci_digest) =
        read_document(&path_of(values, "ci-history"), "workflow run export")?;
    let runs = parse_ci_document(&ci_document).map_err(refusal)?;

    let (commits, git_digest) = match values.get("git-log") {
        Some(log_path) => {
            let git_data = std::fs::read(PathBuf::from(log_path))
                .map_err(|_| "the Git history export could not be read".to_string())?;
            let git_text = String::from_utf8(git_data.clone())
                .map_err(|_| "the Git history export is not UTF-8 text".to_string())?;
            let digest = sha256_bytes(&git_data);
            let commits = parse_git_log(&git_text).map_err(refusal)?;
            (commits, digest)
        }
        None => read_git_history_from_clone(&path_of(values, "git-repo")).map_err(refusal)?,
    };

    let (canary_document, canary_digest) = match values.get("canaryfield-results") {
        Some(path) => {
            let (document, digest) = read_document(&PathBuf::from(path), "canaryfield result")?;
            validate_canary_document(&document).map_err(refusal)?;
            (Some(document), Some(digest))
        }
        None => (None, None),
    };

    // The label is the one argument word that flows into the
    // outputs as data, so a label whose bytes are not UTF-8 text
    // refuses here where the label is resolved, ahead of every
    // write. An empty label and an absent label fall back to the
    // document label, as the Python truthiness reading does.
    let repo = match values.get("repo-label") {
        Some(label) => match label.to_str() {
            Some(text) if !text.is_empty() => text.to_string(),
            Some(_) => ci_repo_label(&ci_document),
            None => return Err("the repository label is not UTF-8 text".to_string()),
        },
        None => ci_repo_label(&ci_document),
    };
    let price = match values.get("cost-per-minute") {
        Some(text) => word_text(text),
        None => "0.008".to_string(),
    };
    let inputs = LedgerInputs {
        repo,
        cost_per_minute: price,
        ci_history_sha256: Some(ci_digest),
        git_history_sha256: Some(git_digest),
        canaryfield_results_sha256: canary_digest,
        canaryfield_document: canary_document,
    };
    let ledger = build_ledger(&runs, &commits, &inputs).map_err(refusal)?;
    let packs = build_evidence_packs(&ledger).map_err(|error| error.message().to_string())?;

    write_text(&path_of(values, "output"), &render_json(&ledger))?;
    let report = render_markdown(&ledger).map_err(refusal)?;
    write_text(&path_of(values, "report"), &report)?;
    let packs_dir = path_of(values, "packs-dir");
    std::fs::create_dir_all(&packs_dir)
        .map_err(|_| "the evidence pack directory could not be created".to_string())?;
    for (filename, pack) in &packs {
        write_text(&packs_dir.join(filename), &render_json(pack))?;
    }
    for (filename, _) in &packs {
        let (written, _digest) = read_document(&packs_dir.join(filename), "written evidence pack")?;
        validate_evidence_pack(&written, &ledger).map_err(|error| error.message().to_string())?;
    }
    Ok(())
}

fn print_stdout(text: &str) {
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    let _ = handle.write_all(text.as_bytes());
    let _ = handle.flush();
}

fn print_stderr(bytes: &[u8]) {
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    let _ = handle.write_all(bytes);
    let _ = handle.flush();
}

fn run(args: &[OsString]) -> u8 {
    match scan(args) {
        Scan::Help => {
            print_stdout(HELP);
            0
        }
        Scan::UsageError => {
            print_stderr(USAGE.as_bytes());
            print_stderr(b"\n");
            2
        }
        Scan::Parsed(values) => match door(&values) {
            Ok(()) => 0,
            Err(message) => {
                print_stderr(format!("catchledger error: {message}\n").as_bytes());
                2
            }
        },
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    ExitCode::from(run(&args))
}
