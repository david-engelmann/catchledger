//! The thin catchledger binary, carrying two doors over the ported
//! core. The pack validation door reads a ledger document and an
//! evidence pack document through the ported reader and validates
//! the pack against the ledger. The report door reads a ledger
//! document and writes its markdown report.

use std::path::Path;
use std::process::ExitCode;

use catchledger::{read_json_file, render_markdown, validate_evidence_pack};

fn fail(message: &str) -> ExitCode {
    eprintln!("catchledger error: {message}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    match args.as_slice() {
        ["validate-pack", ledger_path, pack_path] => {
            let ledger = match read_json_file(Path::new(ledger_path), "ledger") {
                Ok(document) => document,
                Err(error) => return fail(error.message()),
            };
            let pack = match read_json_file(Path::new(pack_path), "evidence pack") {
                Ok(document) => document,
                Err(error) => return fail(error.message()),
            };
            match validate_evidence_pack(&pack, &ledger) {
                Ok(_) => ExitCode::SUCCESS,
                Err(error) => fail(error.message()),
            }
        }
        ["render-report", ledger_path] => {
            let ledger = match read_json_file(Path::new(ledger_path), "ledger") {
                Ok(document) => document,
                Err(error) => return fail(error.message()),
            };
            match render_markdown(&ledger) {
                Ok(markdown) => {
                    print!("{markdown}");
                    ExitCode::SUCCESS
                }
                Err(error) => fail(error.message()),
            }
        }
        _ => {
            eprintln!("usage: catchledger validate-pack LEDGER PACK");
            eprintln!("       catchledger render-report LEDGER");
            ExitCode::from(2)
        }
    }
}
