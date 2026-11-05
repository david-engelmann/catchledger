# catchledger

The scorecard every gate should carry, fires, blocks, true catches, false blocks, flake rate, runtime, and cost per catch, joined from CI logs to what happened next.

## What this is

Dashboards total runs and failures, and no tool anywhere computes which of a team's checks has caught anything lately. Catchledger ingests CI history and git history for one repo and builds one page per check, attributing true catches conservatively from what happened after each block. A block followed by a fix in the flagged code counts as a catch, and a block followed by a bare retry counts against the check. It is the ledger the other gate instruments write their evidence into and the verdict tools read from. The boundary with its sibling is one sentence. Catchledger prices checks, secondbill prices work.

## Status

- Slice 2 is built and proven on synthetic targets only
- Slice 2 ingests a local workflow run export and a local Git history export or clone and builds the per check scorecard with fires and blocks and true catches and false blocks and flake rate and runtime and cost per catch
- Slice 2 attributes a catch only when a later change in the flagged code is followed by a successful run on a descendant commit and counts a block followed by a bare retry against the check and leaves every other block unattributed
- Slice 2 publishes the evidence pack format catchledger-pack/1 with one validated pack for each check and writes a deterministic JSON ledger and markdown report through the command line
- Slice 2 is the Round 1 work increment, and Round 1 closed after audit and two-axis review and red team and owner approval, with the review fixes landed on the same branch
- Round 2 adds the duplicate JSON key refusal, so every JSON document the tool reads refuses a document that carries the same key twice in one object, naming the document and the repeated key
- Round 2 also lands the first Rust port slice, a library crate with a thin binary covering the ledger core and the JSON reader and the canaryfield substrate, proven by 303 recorded conformance cases generated from the Python implementation, which stays in place as the spec
- Round 3 lands the second Rust port slice, carrying workflow run ingestion and Git log parsing and clone reading and the repository label reading and the digest helpers into the crate, proven by 161 new recorded conformance cases beside the standing 303, with the Python implementation standing in place as the spec
- Closed rounds stand at 3 of 100
- Seeded in October 2026. Part of the impatient programming portfolio, in the Gate Truth arc

## Licence

MIT. See LICENSE.
